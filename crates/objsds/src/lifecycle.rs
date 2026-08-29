use objsds_store::{CreateError, Location, ObjectStore, ReplaceError, Version};

use crate::Error;

pub(crate) fn create<S: ObjectStore>(
    store: &S,
    location: &Location,
    bytes: &[u8],
) -> Result<Version, Error<S::Error>> {
    store.create(location, bytes).map_err(|error| match error {
        CreateError::AlreadyExists { observed } => Error::AlreadyExists { observed },
        CreateError::Store(error) => Error::Store(error),
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
            ReplaceError::Conflict { observed } => Error::Conflict { observed },
            ReplaceError::Store(error) => Error::Store(error),
        })
}
