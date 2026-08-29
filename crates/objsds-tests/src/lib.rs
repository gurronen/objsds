use std::error::Error;

mod backend_contract;

pub use backend_contract::assert_backend_contract;

use objsds_store_s3::{Credentials, S3Store};
use s3_client::{AddressingStyle, Auth, BlockingClient, Credentials as S3Credentials};

pub const RUSTFS_ENDPOINT: &str = "http://localhost:9000";
pub const RUSTFS_BUCKET: &str = "objsds-e2e";
pub const RUSTFS_ACCESS_KEY: &str = "rustfsadmin";
pub const RUSTFS_SECRET_KEY: &str = "rustfsadmin";

pub fn rustfs_enabled() -> bool {
    std::env::var("OBJSDS_RUSTFS_E2E").as_deref() == Ok("1")
}

pub fn ensure_rustfs_bucket() -> Result<(), Box<dyn Error>> {
    let credentials = S3Credentials::new(RUSTFS_ACCESS_KEY, RUSTFS_SECRET_KEY)?;
    let client = BlockingClient::builder(RUSTFS_ENDPOINT)?
        .region("us-east-1")
        .auth(Auth::Static(credentials))
        .addressing_style(AddressingStyle::Path)
        .build()?;
    match client.buckets().create(RUSTFS_BUCKET).send() {
        Ok(_) => Ok(()),
        Err(error) if error.status().is_some_and(|status| status.as_u16() == 409) => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}

pub fn rustfs_store() -> Result<S3Store, Box<dyn Error>> {
    Ok(S3Store::builder()
        .bucket(RUSTFS_BUCKET)
        .region("us-east-1")
        .endpoint(RUSTFS_ENDPOINT)
        .credentials(Credentials::new(RUSTFS_ACCESS_KEY, RUSTFS_SECRET_KEY))
        .path_style(true)
        .build()?)
}

pub fn unique_namespace(label: &str) -> String {
    format!("{label}-{}", std::process::id())
}
