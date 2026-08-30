use crate::Credentials;

/// Configuration shared by AWS S3 and compatible object stores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct S3Config {
    /// Bucket containing objects accessed by the adapter.
    pub bucket: String,
    /// Region used for request signing.
    pub region: String,
    /// Explicit S3-compatible endpoint, or `None` for the AWS regional endpoint.
    pub endpoint: Option<String>,
    /// Credential source used to sign requests.
    pub credentials: Credentials,
    /// Whether requests use path-style bucket addressing.
    pub path_style: bool,
}
