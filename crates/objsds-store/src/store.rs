use crate::{CreateError, Location, Object, ReplaceError, Version};

/// Blocking object-store operations required by `objsds`.
///
/// All methods may block the calling thread. Implementations must provide
/// read-after-write consistency for successful writes and atomic conditional
/// semantics for [`create`](Self::create) and [`replace`](Self::replace).
/// Operations transfer complete objects; there are no range, streaming, list,
/// delete, or transaction capabilities.
pub trait ObjectStore: Send + Sync + 'static {
    /// Backend-specific failures unrelated to a conditional-write conflict.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Reads the complete object and its version in one coherent snapshot.
    ///
    /// Returns `Ok(None)` when the location does not exist. This is one
    /// object-store read and allocates memory proportional to object size.
    fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error>;

    /// Creates a complete object only if `location` is absent.
    ///
    /// This is one conditional object-store write in the uncontended case.
    /// Implementations may perform an additional read to report the observed
    /// version after the condition fails.
    fn create(
        &self,
        location: &Location,
        bytes: &[u8],
    ) -> Result<Version, CreateError<Self::Error>>;

    /// Replaces a complete object only when its current version equals
    /// `expected`.
    ///
    /// This is one conditional object-store write in the uncontended case.
    /// Implementations may perform an additional read to populate
    /// [`ReplaceError::Conflict`]. No retry is performed.
    fn replace(
        &self,
        location: &Location,
        expected: &Version,
        bytes: &[u8],
    ) -> Result<Version, ReplaceError<Self::Error>>;
}
