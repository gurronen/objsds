use std::fmt;
use std::sync::Arc;

use objsds_store::{CreateError, Location, Object, ObjectStore, ReplaceError, Version};
use s3_client::BlockingClient;

use crate::{S3Config, S3StoreBuilder, StoreError};

/// Configured blocking S3-compatible object store.
///
/// Clones share a blocking HTTP client. Each [`ObjectStore`] call may block on
/// network I/O and transfers a complete object. Conditional-write failures can
/// perform an additional `GET` to report the version observed afterward.
#[derive(Clone)]
pub struct S3Store {
    pub(crate) config: S3Config,
    pub(crate) client: Arc<BlockingClient>,
}

impl S3Store {
    /// Starts an S3 store builder with no bucket or region.
    #[must_use]
    pub fn builder() -> S3StoreBuilder {
        S3StoreBuilder::default()
    }

    /// Returns the effective configuration, including credential selection.
    ///
    /// Secret values remain redacted by [`Credentials`](crate::Credentials)
    /// debug formatting.
    #[must_use]
    pub fn config(&self) -> &S3Config {
        &self.config
    }

    fn version(etag: Option<String>) -> Result<Version, StoreError> {
        etag.map(Version::new).ok_or(StoreError::MissingEtag)
    }

    fn observed(&self, location: &Location) -> Result<Option<Version>, StoreError> {
        self.get(location)
            .map(|object| object.map(|object| object.version))
    }

    fn is_status(error: &s3_client::Error, expected: u16) -> bool {
        error
            .status()
            .is_some_and(|status| status.as_u16() == expected)
    }
}

impl fmt::Debug for S3Store {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("S3Store")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ObjectStore for S3Store {
    type Error = StoreError;

    fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error> {
        let response = match self
            .client
            .objects()
            .get(&self.config.bucket, location.as_str())
            .send()
        {
            Ok(response) => response,
            Err(error) if Self::is_status(&error, 404) => return Ok(None),
            Err(error) => return Err(StoreError::from(error)),
        };
        let version = Self::version(response.etag.clone())?;
        Ok(Some(Object {
            version,
            bytes: response.bytes()?.to_vec(),
        }))
    }

    fn create(
        &self,
        location: &Location,
        bytes: &[u8],
    ) -> Result<Version, CreateError<Self::Error>> {
        let response = self
            .client
            .objects()
            .put(&self.config.bucket, location.as_str())
            .if_none_match("*")
            .map_err(StoreError::from)
            .map_err(CreateError::Store)?
            .content_type("application/json")
            .map_err(StoreError::from)
            .map_err(CreateError::Store)?
            .body_bytes(bytes.to_vec())
            .send();
        match response {
            Ok(response) => Self::version(response.etag).map_err(CreateError::Store),
            Err(error) if Self::is_status(&error, 409) || Self::is_status(&error, 412) => {
                let observed = self
                    .observed(location)
                    .map_err(CreateError::Store)?
                    .ok_or_else(|| CreateError::Store(StoreError::from(error)))?;
                Err(CreateError::AlreadyExists { observed })
            }
            Err(error) => Err(CreateError::Store(StoreError::from(error))),
        }
    }

    fn replace(
        &self,
        location: &Location,
        expected: &Version,
        bytes: &[u8],
    ) -> Result<Version, ReplaceError<Self::Error>> {
        let response = self
            .client
            .objects()
            .put(&self.config.bucket, location.as_str())
            .if_match(expected.as_str())
            .map_err(StoreError::from)
            .map_err(ReplaceError::Store)?
            .content_type("application/json")
            .map_err(StoreError::from)
            .map_err(ReplaceError::Store)?
            .body_bytes(bytes.to_vec())
            .send();
        match response {
            Ok(response) => Self::version(response.etag).map_err(ReplaceError::Store),
            Err(error)
                if Self::is_status(&error, 404)
                    || Self::is_status(&error, 409)
                    || Self::is_status(&error, 412) =>
            {
                Err(ReplaceError::Conflict {
                    observed: self.observed(location).map_err(ReplaceError::Store)?,
                })
            }
            Err(error) => Err(ReplaceError::Store(StoreError::from(error))),
        }
    }
}
