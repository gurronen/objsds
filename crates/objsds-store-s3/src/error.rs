use std::fmt;

/// Invalid S3 adapter configuration.
#[derive(Debug)]
pub enum BuildError {
    /// No bucket was configured.
    MissingBucket,
    /// No signing region was configured.
    MissingRegion,
    /// The configured region could not be used for signing.
    InvalidRegion,
    /// The blocking client, credentials, or transport could not be configured.
    Transport(Box<s3_client::Error>),
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBucket => formatter.write_str("missing S3 bucket"),
            Self::MissingRegion => formatter.write_str("missing S3 region"),
            Self::InvalidRegion => formatter.write_str("invalid S3 region"),
            Self::Transport(error) => {
                write!(formatter, "could not configure S3 transport: {error}")
            }
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<s3_client::Error> for BuildError {
    fn from(error: s3_client::Error) -> Self {
        Self::Transport(Box::new(error))
    }
}

/// An S3 request or response error.
#[derive(Debug)]
pub enum StoreError {
    /// A request, response body, or request configuration failed.
    Transport(Box<s3_client::Error>),
    /// A successful response omitted the ETag required as a version token.
    MissingEtag,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "S3 request failed: {error}"),
            Self::MissingEtag => formatter.write_str("S3 response did not contain an ETag"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error.as_ref()),
            Self::MissingEtag => None,
        }
    }
}

impl From<s3_client::Error> for StoreError {
    fn from(error: s3_client::Error) -> Self {
        Self::Transport(Box::new(error))
    }
}
