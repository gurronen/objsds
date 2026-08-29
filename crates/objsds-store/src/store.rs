use crate::{CreateError, Location, Object, ReplaceError, Version};

/// Blocking object-store operations required by `objsds`.
pub trait ObjectStore: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error>;

    fn create(
        &self,
        location: &Location,
        bytes: &[u8],
    ) -> Result<Version, CreateError<Self::Error>>;

    fn replace(
        &self,
        location: &Location,
        expected: &Version,
        bytes: &[u8],
    ) -> Result<Version, ReplaceError<Self::Error>>;
}
