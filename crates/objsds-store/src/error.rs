use crate::Version;

/// A conditional create did not succeed.
#[derive(Debug)]
pub enum CreateError<E> {
    /// An object already exists at the location.
    AlreadyExists {
        /// Version observed after the failed create.
        ///
        /// It identifies the object occupying the location at inspection time;
        /// a later read may observe another version.
        observed: Version,
    },
    /// The backing store failed while processing the operation or inspecting
    /// the object after a failed conditional request.
    Store(E),
}

/// A conditional replacement did not succeed.
#[derive(Debug)]
pub enum ReplaceError<E> {
    /// The expected version is no longer current. `None` means the object was
    /// deleted between the read and replacement.
    Conflict {
        /// Version observed after the failed replacement, or `None` if no
        /// object existed at inspection time.
        observed: Option<Version>,
    },
    /// The backing store failed while processing the operation or inspecting
    /// the object after a failed conditional request.
    Store(E),
}
