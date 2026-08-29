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

    use objsds_store::Object;

    use super::*;

    struct ConflictingStore {
        observed: Version,
    }

    impl ObjectStore for ConflictingStore {
        type Error = Infallible;

        fn get(&self, _: &Location) -> Result<Option<Object>, Self::Error> {
            Ok(None)
        }

        fn create(&self, _: &Location, _: &[u8]) -> Result<Version, CreateError<Self::Error>> {
            unreachable!()
        }

        fn replace(
            &self,
            _: &Location,
            _: &Version,
            _: &[u8],
        ) -> Result<Version, ReplaceError<Self::Error>> {
            Err(ReplaceError::Conflict {
                observed: Some(self.observed.clone()),
            })
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
}
