use crate::Credentials;

/// Configuration shared by AWS S3 and compatible object stores.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub credentials: Credentials,
    pub path_style: bool,
}
