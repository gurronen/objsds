use std::sync::Arc;

use s3_client::{AddressingStyle, BlockingClient};

use crate::{BuildError, Credentials, S3Config, S3Store};

/// Builder for [`S3Store`].
#[derive(Clone, Debug, Default)]
pub struct S3StoreBuilder {
    bucket: Option<String>,
    region: Option<String>,
    endpoint: Option<String>,
    credentials: Credentials,
    path_style: bool,
}

impl S3StoreBuilder {
    /// Sets the bucket containing all objects accessed by this store.
    #[must_use]
    pub fn bucket(mut self, bucket: impl Into<String>) -> Self {
        self.bucket = Some(bucket.into());
        self
    }

    /// Sets the signing region (for example, `us-east-1`).
    #[must_use]
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Overrides the service endpoint for S3-compatible providers.
    #[must_use]
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Selects the credentials used to sign requests.
    #[must_use]
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    /// Enables path-style bucket addressing when `true`.
    #[must_use]
    pub fn path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }

    /// Validates configuration and constructs the blocking client.
    ///
    /// Resolving default credentials may read the process environment or other
    /// provider-chain sources. This does not send an object-store request.
    pub fn build(self) -> Result<S3Store, BuildError> {
        let config = S3Config {
            bucket: self.bucket.ok_or(BuildError::MissingBucket)?,
            region: self.region.ok_or(BuildError::MissingRegion)?,
            endpoint: self.endpoint,
            credentials: self.credentials,
            path_style: self.path_style,
        };
        let endpoint = config
            .endpoint
            .clone()
            .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", config.region));
        let addressing = if config.path_style {
            AddressingStyle::Path
        } else {
            AddressingStyle::Auto
        };
        let client = BlockingClient::builder(&endpoint)?
            .region(&config.region)
            .auth(config.credentials.to_auth()?)
            .addressing_style(addressing)
            .build()?;

        Ok(S3Store {
            config,
            client: Arc::new(client),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_rustfs_configuration() {
        let store = S3Store::builder()
            .bucket("data")
            .region("us-east-1")
            .endpoint("http://localhost:9000")
            .credentials(Credentials::new("access", "secret"))
            .path_style(true)
            .build()
            .expect("RustFS configuration should be valid");

        assert_eq!(store.config().bucket, "data");
        assert!(store.config().path_style);
        assert!(!format!("{store:?}").contains("\"secret\""));
    }
}
