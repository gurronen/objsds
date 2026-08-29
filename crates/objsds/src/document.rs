use serde::Deserialize;

use crate::Error;

pub(crate) const FORMAT_VERSION: u32 = 1;

#[derive(Deserialize)]
struct Metadata {
    format_version: u32,
    kind: String,
    schema: String,
}

pub(crate) fn validate<E>(bytes: &[u8], kind: &str, schema: &str) -> Result<(), Error<E>> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    if metadata.format_version != FORMAT_VERSION {
        return Err(Error::Incompatible {
            expected: format!("format version {FORMAT_VERSION}"),
            observed: format!("format version {}", metadata.format_version),
        });
    }
    if metadata.kind != kind {
        return Err(Error::Incompatible {
            expected: kind.to_owned(),
            observed: metadata.kind,
        });
    }
    if metadata.schema != schema {
        return Err(Error::Incompatible {
            expected: format!("schema {schema}"),
            observed: format!("schema {}", metadata.schema),
        });
    }
    Ok(())
}
