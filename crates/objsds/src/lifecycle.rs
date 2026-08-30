use objsds_store::{CreateError, Location, ObjectStore, ReplaceError, Version};

use crate::{AlreadyExistsError, ConflictError, Error, StoreError};

pub(crate) fn create<S: ObjectStore>(
    store: &S,
    location: &Location,
    bytes: &[u8],
) -> Result<Version, Error<S::Error>> {
    store.create(location, bytes).map_err(|error| match error {
        CreateError::AlreadyExists { observed } => {
            Error::AlreadyExists(AlreadyExistsError { observed })
        }
        CreateError::Store(source) => Error::Store(StoreError { source }),
    })
}

pub(crate) fn create_structure<S, B>(
    store: &S,
    location: &Location,
    empty_bytes: B,
) -> Result<(), Error<S::Error>>
where
    S: ObjectStore,
    B: FnOnce() -> Result<Vec<u8>, Error<S::Error>>,
{
    let bytes = empty_bytes()?;
    create(store, location, &bytes)?;
    Ok(())
}

pub(crate) fn open_structure<S, R, T>(read: R) -> Result<(), Error<S::Error>>
where
    S: ObjectStore,
    R: FnOnce() -> Result<T, Error<S::Error>>,
{
    read()?;
    Ok(())
}

pub(crate) fn open_or_create_structure<S, B, R, T>(
    store: &S,
    location: &Location,
    empty_bytes: B,
    read: R,
) -> Result<(), Error<S::Error>>
where
    S: ObjectStore,
    B: FnOnce() -> Result<Vec<u8>, Error<S::Error>>,
    R: FnOnce() -> Result<T, Error<S::Error>>,
{
    if store
        .get(location)
        .map_err(|source| Error::Store(StoreError { source }))?
        .is_some()
    {
        return open_structure::<S, _, _>(read);
    }

    let bytes = empty_bytes()?;
    match create(store, location, &bytes) {
        Ok(_) => Ok(()),
        Err(Error::AlreadyExists(AlreadyExistsError { .. })) => open_structure::<S, _, _>(read),
        Err(error) => Err(error),
    }
}

pub(crate) fn replace<S: ObjectStore>(
    store: &S,
    location: &Location,
    expected: &Version,
    bytes: &[u8],
) -> Result<Version, Error<S::Error>> {
    store
        .replace(location, expected, bytes)
        .map_err(|error| match error {
            ReplaceError::Conflict { observed } => Error::Conflict(ConflictError {
                expected: expected.clone(),
                observed,
            }),
            ReplaceError::Store(source) => Error::Store(StoreError { source }),
        })
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::sync::atomic::{AtomicBool, Ordering};

    use objsds_store::Object;
    use objsds_store_memory::MemoryStore;

    use super::*;

    struct ConflictingStore {
        observed: Version,
    }

    impl ObjectStore for ConflictingStore {
        type Error = Infallible;

        fn get(&self, _: &Location) -> std::result::Result<Option<Object>, Self::Error> {
            Ok(None)
        }

        fn create(
            &self,
            _: &Location,
            _: &[u8],
        ) -> std::result::Result<Version, CreateError<Self::Error>> {
            unreachable!()
        }

        fn replace(
            &self,
            _: &Location,
            _: &Version,
            _: &[u8],
        ) -> std::result::Result<Version, ReplaceError<Self::Error>> {
            Err(ReplaceError::Conflict {
                observed: Some(self.observed.clone()),
            })
        }
    }

    struct CreateRaceStore {
        inner: MemoryStore,
        raced: AtomicBool,
    }

    impl ObjectStore for CreateRaceStore {
        type Error = Infallible;

        fn get(&self, location: &Location) -> std::result::Result<Option<Object>, Self::Error> {
            if !self.raced.swap(true, Ordering::SeqCst) {
                self.inner
                    .create(location, b"winner")
                    .expect("competing create should succeed");
                return Ok(None);
            }
            self.inner.get(location)
        }

        fn create(
            &self,
            location: &Location,
            bytes: &[u8],
        ) -> std::result::Result<Version, CreateError<Self::Error>> {
            self.inner.create(location, bytes)
        }

        fn replace(
            &self,
            location: &Location,
            expected: &Version,
            bytes: &[u8],
        ) -> std::result::Result<Version, ReplaceError<Self::Error>> {
            self.inner.replace(location, expected, bytes)
        }
    }

    #[test]
    fn replacement_conflict_reports_expected_and_observed_versions() {
        let expected = Version::new("expected");
        let observed = Version::new("observed");
        let location = Location::new("maps/users.json").expect("location should be valid");

        let error = replace(
            &ConflictingStore {
                observed: observed.clone(),
            },
            &location,
            &expected,
            b"replacement",
        )
        .expect_err("replacement should conflict");

        assert!(matches!(
            error,
            Error::Conflict(ConflictError {
                expected: actual_expected,
                observed: Some(actual_observed),
            }) if actual_expected == expected && actual_observed == observed
        ));
    }

    #[test]
    fn open_or_create_opens_the_winner_of_a_create_race() {
        let store = CreateRaceStore {
            inner: MemoryStore::default(),
            raced: AtomicBool::new(false),
        };
        let location = Location::new("maps/users.json").expect("location should be valid");

        open_or_create_structure(
            &store,
            &location,
            || Ok(b"loser".to_vec()),
            || {
                let object = store
                    .get(&location)
                    .map_err(|source| Error::Store(StoreError { source }))?
                    .expect("winning object should exist");
                assert_eq!(object.bytes, b"winner");
                Ok(())
            },
        )
        .expect("the winning object should open");
    }
}
