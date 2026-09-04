//! A blocking, brokerless work queue stored in one object.
//!
//! Every operation reads the complete JSON document. Mutations conditionally
//! replace it using the [`objsds_store::ObjectStore`] version token, so
//! concurrent mutations can return [`Error::Conflict`] and are never retried
//! automatically. CPU, memory, and transfer costs are O(n) in queue size.
//!
//! Successful publication makes a message available at least once. A claim
//! grants one time-bounded lease; an unacknowledged message becomes claimable
//! again when that lease expires. Acknowledgement permanently removes the
//! message only when its current lease token matches and has not expired.
//! [`Ack::NotFound`], [`Ack::LeaseMismatch`], and [`Ack::LeaseExpired`] are
//! classifications of the snapshot that was read; they are not written back.
//! Only [`Ack::Acknowledged`] is made durable with a successful compare-and-swap.
//! Consequently handlers must be idempotent: a worker can finish its side
//! effect and fail before acknowledgement, and lease expiry can allow another
//! worker to process the same message. There is no exactly-once delivery,
//! broker, consumer group, group commit, long polling, or automatic retry.
//! Clock synchronization bounds lease safety across processes.

#![deny(missing_docs)]

use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use objsds_store::{CreateError, Location, ObjectStore, ReplaceError, Version, is_path_segment};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const FORMAT_VERSION: u32 = 1;
const KIND: &str = "queue";

/// Supplies Unix time in milliseconds for lease decisions.
///
/// Production clocks used by different workers must be sufficiently
/// synchronized for the chosen lease duration. Tests can provide a manually
/// advanced implementation.
pub trait Clock: Send + Sync + 'static {
    /// Returns milliseconds elapsed since the Unix epoch.
    fn now_millis(&self) -> u64;
}

/// Wall clock used by default.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        u64::try_from(millis).unwrap_or(u64::MAX)
    }
}

/// Opaque identifier assigned to one published message.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct MessageId(Uuid);

impl fmt::Display for MessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Opaque token authorizing acknowledgement of one claim.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct LeaseToken(Uuid);

impl fmt::Display for LeaseToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A message claimed for processing until its lease deadline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim<V> {
    /// Stable identifier of the published message.
    pub id: MessageId,
    /// Application payload.
    pub value: V,
    /// Token required to acknowledge this particular delivery attempt.
    pub lease_token: LeaseToken,
    /// One-based number of times this message has been claimed.
    pub attempt: u32,
    /// Exclusive Unix-millisecond lease deadline.
    pub lease_expires_at_millis: u64,
}

/// Outcome of an acknowledgement attempt.
///
/// Negative variants are classified from the snapshot that was read and do not
/// perform a write. A concurrent claim or acknowledgement can make them stale.
/// Only [`Ack::Acknowledged`] is durable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ack {
    /// The current claim was acknowledged and the message was removed.
    Acknowledged,
    /// No queued message has that identifier in the snapshot that was read.
    NotFound,
    /// The snapshot's lease token does not match, or the message has no lease.
    LeaseMismatch,
    /// The matching lease has expired in the snapshot that was read.
    LeaseExpired,
}

/// Queue construction error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildError {
    /// Namespace was empty or was not one path segment.
    InvalidNamespace,
    /// Queue name was empty or was not one path segment.
    InvalidName,
    /// No non-empty stable schema identifier was supplied.
    MissingSchema,
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidNamespace => "namespace must be one non-empty path segment",
            Self::InvalidName => "queue name must be one non-empty path segment",
            Self::MissingSchema => "missing schema identifier",
        })
    }
}

impl std::error::Error for BuildError {}

/// A queue operation error.
#[derive(Debug)]
pub enum Error<E> {
    /// Queue configuration is invalid.
    Configuration(BuildError),
    /// The object store failed independently of a conditional-write conflict.
    Store(E),
    /// JSON encoding or decoding failed.
    Document(serde_json::Error),
    /// The stored document violates a queue invariant.
    Corrupt(String),
    /// The queue object does not exist.
    NotFound,
    /// Conditional creation found an existing object.
    AlreadyExists(Version),
    /// A mutation lost a compare-and-swap race.
    Conflict {
        /// Version read before the attempted mutation.
        expected: Version,
        /// Version observed after failure, if the object still existed.
        observed: Option<Version>,
    },
    /// Stored format version, structure kind, or schema is incompatible.
    Incompatible(String),
    /// Lease duration is zero, sub-millisecond, or cannot be represented.
    InvalidLeaseDuration,
    /// The claim count cannot be incremented further.
    AttemptOverflow,
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(error) => write!(formatter, "invalid configuration: {error}"),
            Self::Store(error) => write!(formatter, "object store error: {error}"),
            Self::Document(error) => write!(formatter, "could not process queue JSON: {error}"),
            Self::Corrupt(reason) => write!(formatter, "persisted queue is corrupt: {reason}"),
            Self::NotFound => formatter.write_str("queue does not exist"),
            Self::AlreadyExists(_) => formatter.write_str("queue already exists"),
            Self::Conflict { .. } => formatter.write_str("object version conflict"),
            Self::Incompatible(reason) => write!(formatter, "incompatible queue: {reason}"),
            Self::InvalidLeaseDuration => formatter.write_str("invalid lease duration"),
            Self::AttemptOverflow => formatter.write_str("message claim count overflowed"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Document(error) => Some(error),
            _ => None,
        }
    }
}

/// Configures and opens one queue.
pub struct QueueBuilder<S, V, C = SystemClock> {
    store: Arc<S>,
    namespace: String,
    name: String,
    schema: Option<String>,
    clock: C,
    value: PhantomData<fn() -> V>,
}

impl<S, V> QueueBuilder<S, V, SystemClock> {
    /// Starts a builder for a queue in `namespace` with `name`.
    ///
    /// The builder takes ownership of `store` and shares it with the resulting
    /// queue through an [`Arc`], so `S` does not need to implement [`Clone`].
    pub fn new(store: S, namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            store: Arc::new(store),
            namespace: namespace.into(),
            name: name.into(),
            schema: None,
            clock: SystemClock,
            value: PhantomData,
        }
    }
}

impl<S, V, C> QueueBuilder<S, V, C> {
    /// Sets the stable application-defined payload schema identifier.
    #[must_use]
    pub fn schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    /// Replaces the clock used for lease decisions.
    #[must_use]
    pub fn clock<D: Clock>(self, clock: D) -> QueueBuilder<S, V, D> {
        QueueBuilder {
            store: self.store,
            namespace: self.namespace,
            name: self.name,
            schema: self.schema,
            clock,
            value: PhantomData,
        }
    }

    fn finish(self) -> Result<Queue<S, V, C>, BuildError> {
        if !is_path_segment(&self.namespace) {
            return Err(BuildError::InvalidNamespace);
        }
        if !is_path_segment(&self.name) {
            return Err(BuildError::InvalidName);
        }
        let schema = self
            .schema
            .filter(|schema| !schema.is_empty())
            .ok_or(BuildError::MissingSchema)?;
        let location = Location::structure(&self.namespace, "queues", &self.name)
            .map_err(|_| BuildError::InvalidName)?;
        Ok(Queue {
            store: self.store,
            location,
            schema,
            clock: self.clock,
            value: PhantomData,
        })
    }
}

impl<S, V, C> QueueBuilder<S, V, C>
where
    S: ObjectStore,
    V: Serialize + DeserializeOwned,
    C: Clock,
{
    /// Conditionally creates an empty queue.
    pub fn create(self) -> Result<Queue<S, V, C>, Error<S::Error>> {
        let queue = self.finish().map_err(Error::Configuration)?;
        let bytes = queue.empty_bytes()?;
        queue
            .store
            .create(&queue.location, &bytes)
            .map_err(map_create)?;
        Ok(queue)
    }

    /// Opens and validates an existing queue.
    pub fn open(self) -> Result<Queue<S, V, C>, Error<S::Error>> {
        let queue = self.finish().map_err(Error::Configuration)?;
        queue.read()?;
        Ok(queue)
    }

    /// Opens a valid queue or atomically creates it when absent.
    pub fn open_or_create(self) -> Result<Queue<S, V, C>, Error<S::Error>> {
        let queue = self.finish().map_err(Error::Configuration)?;
        if queue
            .store
            .get(&queue.location)
            .map_err(Error::Store)?
            .is_some()
        {
            queue.read()?;
            return Ok(queue);
        }
        let bytes = queue.empty_bytes()?;
        match queue.store.create(&queue.location, &bytes) {
            Ok(_) => Ok(queue),
            Err(CreateError::AlreadyExists { .. }) => {
                queue.read()?;
                Ok(queue)
            }
            Err(CreateError::Store(error)) => Err(Error::Store(error)),
        }
    }
}

/// A brokerless queue occupying exactly one object.
///
/// The object store is retained through an [`Arc`], matching the sharing model
/// used by other objsds structure handles.
pub struct Queue<S, V, C = SystemClock> {
    store: Arc<S>,
    location: Location,
    schema: String,
    clock: C,
    value: PhantomData<fn() -> V>,
}

impl<S, V, C> Queue<S, V, C>
where
    S: ObjectStore,
    V: Serialize + DeserializeOwned,
    C: Clock,
{
    /// Publishes a message and returns its stable identifier.
    ///
    /// A conflict means the message was not applied. A store error can be
    /// ambiguous; retrying with a new generated ID can therefore duplicate a
    /// message.
    pub fn publish(&self, value: V) -> Result<MessageId, Error<S::Error>> {
        let (version, mut document) = self.read()?;
        let id = MessageId(Uuid::now_v7());
        let index = document.messages.partition_point(|message| message.id < id);
        document.messages.insert(
            index,
            Message {
                id,
                value,
                attempts: 0,
                lease: None,
            },
        );
        self.write(&version, &document)?;
        Ok(id)
    }

    /// Claims the oldest available message for the requested lease duration.
    ///
    /// A message is available when never claimed or when its prior lease's
    /// exclusive deadline is at or before the current clock value. Returns
    /// `None` without writing if no message is available.
    pub fn claim(&self, lease_duration: Duration) -> Result<Option<Claim<V>>, Error<S::Error>>
    where
        V: Clone,
    {
        let lease_millis = u64::try_from(lease_duration.as_millis())
            .ok()
            .filter(|millis| *millis > 0)
            .ok_or(Error::InvalidLeaseDuration)?;
        let now = self.clock.now_millis();
        let expires_at_millis = now
            .checked_add(lease_millis)
            .ok_or(Error::InvalidLeaseDuration)?;
        let (version, mut document) = self.read()?;
        let Some(message) = document.messages.iter_mut().find(|message| {
            message
                .lease
                .as_ref()
                .is_none_or(|lease| lease.expires_at_millis <= now)
        }) else {
            return Ok(None);
        };
        message.attempts = message
            .attempts
            .checked_add(1)
            .ok_or(Error::AttemptOverflow)?;
        let token = LeaseToken(Uuid::now_v7());
        message.lease = Some(Lease {
            token,
            expires_at_millis,
        });
        let claim = Claim {
            id: message.id,
            value: message.value.clone(),
            lease_token: token,
            attempt: message.attempts,
            lease_expires_at_millis: expires_at_millis,
        };
        self.write(&version, &document)?;
        Ok(Some(claim))
    }

    /// Acknowledges and removes a message when the current unexpired lease matches.
    ///
    /// [`Ack::Acknowledged`] is the only durable outcome. Other [`Ack`] values
    /// return without writing, so callers must not treat them as a stable
    /// terminal state without reading a fresh snapshot or claiming again.
    pub fn ack(&self, id: MessageId, token: LeaseToken) -> Result<Ack, Error<S::Error>> {
        let now = self.clock.now_millis();
        let (version, mut document) = self.read()?;
        let Ok(index) = document
            .messages
            .binary_search_by_key(&id, |message| message.id)
        else {
            return Ok(Ack::NotFound);
        };
        let Some(lease) = document.messages[index].lease.as_ref() else {
            return Ok(Ack::LeaseMismatch);
        };
        if lease.token != token {
            return Ok(Ack::LeaseMismatch);
        }
        if lease.expires_at_millis <= now {
            return Ok(Ack::LeaseExpired);
        }
        document.messages.remove(index);
        self.write(&version, &document)?;
        Ok(Ack::Acknowledged)
    }

    /// Returns the number of unacknowledged messages in one snapshot.
    pub fn len(&self) -> Result<usize, Error<S::Error>> {
        Ok(self.read()?.1.messages.len())
    }

    /// Returns whether the queue has no unacknowledged messages.
    pub fn is_empty(&self) -> Result<bool, Error<S::Error>> {
        Ok(self.len()? == 0)
    }

    fn empty_bytes(&self) -> Result<Vec<u8>, Error<S::Error>> {
        serde_json::to_vec(&Document::<V> {
            format_version: FORMAT_VERSION,
            kind: KIND.to_owned(),
            schema: self.schema.clone(),
            messages: Vec::new(),
        })
        .map_err(Error::Document)
    }

    fn read(&self) -> Result<(Version, Document<V>), Error<S::Error>> {
        let object = self
            .store
            .get(&self.location)
            .map_err(Error::Store)?
            .ok_or(Error::NotFound)?;
        let document: Document<V> =
            serde_json::from_slice(&object.bytes).map_err(Error::Document)?;
        if document.format_version != FORMAT_VERSION {
            return Err(Error::Incompatible(format!(
                "expected format version {FORMAT_VERSION}, observed {}",
                document.format_version
            )));
        }
        if document.kind != KIND {
            return Err(Error::Incompatible(format!(
                "expected kind {KIND}, observed {}",
                document.kind
            )));
        }
        if document.schema != self.schema {
            return Err(Error::Incompatible(format!(
                "expected schema {}, observed {}",
                self.schema, document.schema
            )));
        }
        if document
            .messages
            .windows(2)
            .any(|messages| messages[0].id >= messages[1].id)
        {
            return Err(Error::Corrupt(
                "message IDs must be strictly increasing".to_owned(),
            ));
        }
        Ok((object.version, document))
    }

    fn write(&self, expected: &Version, document: &Document<V>) -> Result<(), Error<S::Error>> {
        let bytes = serde_json::to_vec(document).map_err(Error::Document)?;
        self.store
            .replace(&self.location, expected, &bytes)
            .map_err(|error| match error {
                ReplaceError::Conflict { observed } => Error::Conflict {
                    expected: expected.clone(),
                    observed,
                },
                ReplaceError::Store(error) => Error::Store(error),
            })?;
        Ok(())
    }
}

fn map_create<E>(error: CreateError<E>) -> Error<E> {
    match error {
        CreateError::AlreadyExists { observed } => Error::AlreadyExists(observed),
        CreateError::Store(error) => Error::Store(error),
    }
}

#[derive(Deserialize, Serialize)]
struct Document<V> {
    format_version: u32,
    kind: String,
    schema: String,
    messages: Vec<Message<V>>,
}

#[derive(Deserialize, Serialize)]
struct Message<V> {
    id: MessageId,
    value: V,
    attempts: u32,
    lease: Option<Lease>,
}

#[derive(Deserialize, Serialize)]
struct Lease {
    token: LeaseToken,
    expires_at_millis: u64,
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use objsds_store::{Object, ObjectStore, ReplaceError};
    use objsds_store_memory::MemoryStore;

    use super::*;

    #[derive(Clone)]
    struct ManualClock(Arc<AtomicU64>);

    impl ManualClock {
        fn new(now: u64) -> Self {
            Self(Arc::new(AtomicU64::new(now)))
        }
        fn set(&self, now: u64) {
            self.0.store(now, Ordering::SeqCst);
        }
    }

    impl Clock for ManualClock {
        fn now_millis(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn queue(clock: ManualClock) -> Queue<MemoryStore, String, ManualClock> {
        QueueBuilder::new(MemoryStore::default(), "tests", "jobs")
            .schema("job-v1")
            .clock(clock)
            .create()
            .expect("queue should be created")
    }

    #[test]
    fn publish_claim_and_ack_removes_message() {
        let queue = queue(ManualClock::new(1_000));
        let id = queue
            .publish("work".to_owned())
            .expect("publish should succeed");
        let claim = queue
            .claim(Duration::from_millis(100))
            .expect("claim should succeed")
            .expect("message should be available");
        assert_eq!(claim.id, id);
        assert_eq!(claim.value, "work");
        assert_eq!(claim.attempt, 1);
        assert_eq!(
            queue
                .ack(id, claim.lease_token)
                .expect("ack should succeed"),
            Ack::Acknowledged
        );
        assert!(queue.is_empty().expect("read should succeed"));
    }

    #[test]
    fn lease_expiry_reclaims_with_new_token_and_attempt() {
        let clock = ManualClock::new(1_000);
        let queue = queue(clock.clone());
        let id = queue
            .publish("work".to_owned())
            .expect("publish should succeed");
        let first = queue
            .claim(Duration::from_millis(100))
            .expect("claim should succeed")
            .expect("message should be available");
        assert!(
            queue
                .claim(Duration::from_millis(100))
                .expect("claim should succeed")
                .is_none()
        );
        clock.set(1_100);
        assert_eq!(
            queue
                .ack(id, first.lease_token)
                .expect("ack should be classified"),
            Ack::LeaseExpired
        );
        let second = queue
            .claim(Duration::from_millis(100))
            .expect("reclaim should succeed")
            .expect("expired message should be available");
        assert_eq!(second.id, id);
        assert_eq!(second.attempt, 2);
        assert_ne!(second.lease_token, first.lease_token);
        assert_eq!(
            queue
                .ack(id, first.lease_token)
                .expect("stale ack should be classified"),
            Ack::LeaseMismatch
        );
        assert_eq!(
            queue
                .ack(id, second.lease_token)
                .expect("ack should succeed"),
            Ack::Acknowledged
        );
    }

    #[test]
    fn claims_available_messages_in_publish_order() {
        let queue = queue(ManualClock::new(1_000));
        let first = queue
            .publish("first".to_owned())
            .expect("publish should succeed");
        let second = queue
            .publish("second".to_owned())
            .expect("publish should succeed");
        let first_claim = queue
            .claim(Duration::from_secs(1))
            .expect("claim should succeed")
            .expect("first message should exist");
        let second_claim = queue
            .claim(Duration::from_secs(1))
            .expect("claim should succeed")
            .expect("second message should exist");
        assert_eq!((first_claim.id, second_claim.id), (first, second));
    }

    #[derive(Clone)]
    struct FailReplaceOnce {
        inner: MemoryStore,
        fail_next: Arc<AtomicBool>,
    }

    impl ObjectStore for FailReplaceOnce {
        type Error = Infallible;

        fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error> {
            self.inner.get(location)
        }

        fn create(
            &self,
            location: &Location,
            bytes: &[u8],
        ) -> Result<Version, CreateError<Self::Error>> {
            self.inner.create(location, bytes)
        }

        fn replace(
            &self,
            location: &Location,
            expected: &Version,
            bytes: &[u8],
        ) -> Result<Version, ReplaceError<Self::Error>> {
            if self.fail_next.swap(false, Ordering::SeqCst) {
                let observed = self
                    .inner
                    .get(location)
                    .expect("memory get is infallible")
                    .map(|object| object.version);
                return Err(ReplaceError::Conflict { observed });
            }
            self.inner.replace(location, expected, bytes)
        }
    }

    #[test]
    fn publish_conflict_leaves_queue_unchanged() {
        let store = FailReplaceOnce {
            inner: MemoryStore::default(),
            fail_next: Arc::new(AtomicBool::new(false)),
        };
        let queue = QueueBuilder::new(store.clone(), "tests", "jobs")
            .schema("job-v1")
            .create()
            .expect("queue should be created");
        store.fail_next.store(true, Ordering::SeqCst);
        assert!(matches!(
            queue.publish("work".to_owned()),
            Err(Error::Conflict { .. })
        ));
        assert!(queue.is_empty().expect("read should succeed"));
    }

    #[test]
    fn open_validates_existing_queue_and_schema() {
        let store = MemoryStore::default();
        QueueBuilder::<_, String>::new(store.clone(), "tests", "jobs")
            .schema("job-v1")
            .create()
            .expect("queue should be created");
        QueueBuilder::<_, String>::new(store.clone(), "tests", "jobs")
            .schema("job-v1")
            .open()
            .expect("existing queue should open");
        assert!(matches!(
            QueueBuilder::<_, String>::new(store.clone(), "tests", "jobs")
                .schema("job-v2")
                .open(),
            Err(Error::Incompatible(_))
        ));
        assert!(matches!(
            QueueBuilder::<_, String>::new(store, "tests", "missing")
                .schema("job-v1")
                .open(),
            Err(Error::NotFound)
        ));
    }

    #[test]
    fn open_or_create_is_idempotent() {
        let store = MemoryStore::default();
        let created = QueueBuilder::<_, String>::new(store.clone(), "tests", "jobs")
            .schema("job-v1")
            .open_or_create()
            .expect("queue should be created");
        created
            .publish("work".to_owned())
            .expect("publish should succeed");
        let opened = QueueBuilder::<_, String>::new(store, "tests", "jobs")
            .schema("job-v1")
            .open_or_create()
            .expect("existing queue should open");
        assert_eq!(opened.len().expect("read should succeed"), 1);
    }

    #[test]
    fn rejects_zero_and_sub_millisecond_leases() {
        let queue = queue(ManualClock::new(1_000));
        assert!(matches!(
            queue.claim(Duration::ZERO),
            Err(Error::InvalidLeaseDuration)
        ));
        assert!(matches!(
            queue.claim(Duration::from_nanos(1)),
            Err(Error::InvalidLeaseDuration)
        ));
    }

    #[test]
    fn ack_classifies_missing_and_unclaimed_messages() {
        let queue = queue(ManualClock::new(1_000));
        let token = LeaseToken(Uuid::nil());
        assert_eq!(
            queue
                .ack(MessageId(Uuid::nil()), token)
                .expect("ack should be classified"),
            Ack::NotFound
        );
        let id = queue
            .publish("work".to_owned())
            .expect("publish should succeed");
        assert_eq!(
            queue.ack(id, token).expect("ack should be classified"),
            Ack::LeaseMismatch
        );
    }
}
