use std::fmt;

use objsds_store::Version;

use crate::BuildError;

/// An `objsds` operation error.
#[derive(Debug)]
pub enum Error<E> {
    /// The builder was not configured correctly.
    Configuration(BuildError),
    /// The object store could not complete the operation.
    Store(StoreError<E>),
    /// A document could not be encoded or decoded.
    Document(DocumentError),
    /// The requested data structure does not exist.
    NotFound,
    /// Creation found an existing object at the structure's location.
    AlreadyExists(AlreadyExistsError),
    /// A mutation lost a compare-and-swap race.
    ///
    /// The operation was not applied and is not retried automatically. Inspect
    /// the expected and observed versions for diagnostics, but do not treat the
    /// observation as a safe token for a replacement without reading a fresh
    /// snapshot.
    ///
    /// ```
    /// use std::convert::Infallible;
    /// use objsds::{ConflictError, Error, Version};
    ///
    /// let error: Error<Infallible> = Error::Conflict(ConflictError {
    ///     expected: Version::new("etag-read-before-write"),
    ///     observed: Some(Version::new("etag-after-conflict")),
    /// });
    /// match error {
    ///     Error::Conflict(ConflictError { expected, observed: Some(observed) }) => {
    ///         eprintln!("expected {expected:?}, observed {observed:?}");
    ///     }
    ///     Error::Conflict(ConflictError { expected, observed: None }) => {
    ///         eprintln!("expected {expected:?}, but the object was absent");
    ///     }
    ///     _ => unreachable!(),
    /// }
    /// ```
    Conflict(ConflictError),
    /// Persisted metadata is incompatible with the requested structure.
    Incompatible(CompatibilityError),
}

/// The object store could not complete an operation.
#[derive(Debug)]
pub struct StoreError<E> {
    /// Backend-specific source error.
    pub source: E,
}

/// A document could not be encoded or decoded.
#[derive(Debug)]
pub enum DocumentError {
    /// A value supplied by the application could not be encoded.
    Serialize(serde_json::Error),
    /// The persisted document is malformed or cannot be decoded as the expected type.
    Deserialize(serde_json::Error),
    /// The decoded document violates a data-structure invariant.
    Corrupt {
        /// Human-readable description of the violated invariant.
        reason: String,
    },
}

/// Creation found an object already present at the requested location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlreadyExistsError {
    /// Version observed after the failed conditional create.
    ///
    /// This is diagnostic and may already be stale.
    pub observed: Version,
}

/// A conditional replacement did not observe the expected object version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictError {
    /// Version read from the snapshot that the mutation attempted to replace.
    pub expected: Version,
    /// Version observed after the failed replacement, or `None` if the object
    /// was absent. This observation is diagnostic and may already be stale.
    pub observed: Option<Version>,
}

/// Persisted metadata is incompatible with the requested structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibilityError {
    /// The stored format version is unsupported.
    FormatVersion {
        /// Format version supported by this library.
        expected: u32,
        /// Format version found in the stored snapshot.
        observed: u32,
    },
    /// The stored structure kind differs from the requested kind.
    Kind {
        /// Requested structure kind.
        expected: String,
        /// Structure kind found in the stored snapshot.
        observed: String,
    },
    /// The stored application schema identifier differs from the requested one.
    Schema {
        /// Schema identifier configured by the caller.
        expected: String,
        /// Schema identifier found in the stored snapshot.
        observed: String,
    },
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(error) => write!(formatter, "invalid configuration: {error}"),
            Self::Store(error) => write!(formatter, "object store error: {}", error.source),
            Self::Document(error) => error.fmt(formatter),
            Self::NotFound => formatter.write_str("data structure does not exist"),
            Self::AlreadyExists(_) => formatter.write_str("data structure already exists"),
            Self::Conflict(_) => formatter.write_str("object version conflict"),
            Self::Incompatible(error) => error.fmt(formatter),
        }
    }
}

impl<E: fmt::Display> fmt::Display for StoreError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl fmt::Display for AlreadyExistsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "object already exists at version {}",
            self.observed.as_str()
        )
    }
}

impl fmt::Display for ConflictError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.observed {
            Some(observed) => write!(
                formatter,
                "expected object version {}, observed {}",
                self.expected.as_str(),
                observed.as_str()
            ),
            None => write!(
                formatter,
                "expected object version {}, but the object was deleted",
                self.expected.as_str()
            ),
        }
    }
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(error) => write!(formatter, "could not encode JSON document: {error}"),
            Self::Deserialize(error) => {
                write!(formatter, "persisted JSON document is malformed: {error}")
            }
            Self::Corrupt { reason } => {
                write!(formatter, "persisted document is corrupt: {reason}")
            }
        }
    }
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("incompatible data structure: ")?;
        match self {
            Self::FormatVersion { expected, observed } => {
                write!(
                    formatter,
                    "expected format version {expected}, observed {observed}"
                )
            }
            Self::Kind { expected, observed } => {
                write!(formatter, "expected kind {expected}, observed {observed}")
            }
            Self::Schema { expected, observed } => {
                write!(formatter, "expected schema {expected}, observed {observed}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::Store(error) => Some(&error.source),
            Self::Document(error) => Some(error),
            _ => None,
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for StoreError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl std::error::Error for AlreadyExistsError {}
impl std::error::Error for ConflictError {}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialize(error) | Self::Deserialize(error) => Some(error),
            Self::Corrupt { .. } => None,
        }
    }
}

impl std::error::Error for CompatibilityError {}
