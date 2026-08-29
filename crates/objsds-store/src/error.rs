use crate::Version;

/// A conditional create did not succeed.
#[derive(Debug)]
pub enum CreateError<E> {
    /// An object already exists at the location.
    AlreadyExists {
        observed: Version,
    },
    Store(E),
}

/// A conditional replacement did not succeed.
#[derive(Debug)]
pub enum ReplaceError<E> {
    /// The expected version is no longer current. `None` means the object was
    /// deleted between the read and replacement.
    Conflict {
        observed: Option<Version>,
    },
    Store(E),
}
