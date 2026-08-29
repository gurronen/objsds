use objsds::Objsds;
use objsds_store_s3::{Credentials, S3Store};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct User {
    name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = S3Store::builder()
        .bucket("objsds-e2e")
        .region("us-east-1")
        .endpoint("http://localhost:9000")
        .credentials(Credentials::new("rustfsadmin", "rustfsadmin"))
        .path_style(true)
        .build()?;
    let client = Objsds::builder()
        .store(store)
        .namespace("rustfs-example")
        .build()?;
    let users = client
        .map::<User>("users")
        .schema("user-v1")
        .open_or_create()?;

    users.insert(
        "alice",
        User {
            name: "Alice".to_owned(),
        },
    )?;
    println!("{:?}", users.get("alice")?);
    Ok(())
}
