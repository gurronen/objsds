use objsds_store::{Object, Version};
use uuid::Uuid;

use crate::StoreError;

const MAGIC: &[u8; 8] = b"OBJSDSFS";
const FORMAT_VERSION: u8 = 1;
const HEADER_LEN: usize = 33;

pub(crate) fn encode(bytes: &[u8]) -> (Version, Vec<u8>) {
    let revision = Uuid::now_v7();
    let version = Version::new(revision.to_string());
    let mut encoded = Vec::with_capacity(HEADER_LEN + bytes.len());
    encoded.extend_from_slice(MAGIC);
    encoded.push(FORMAT_VERSION);
    encoded.extend_from_slice(revision.as_bytes());
    encoded.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    encoded.extend_from_slice(bytes);
    (version, encoded)
}

pub(crate) fn decode(encoded: &[u8]) -> Result<Object, StoreError> {
    if encoded.len() < HEADER_LEN || &encoded[..8] != MAGIC {
        return Err(StoreError::InvalidObjectFormat);
    }
    if encoded[8] != FORMAT_VERSION {
        return Err(StoreError::UnsupportedFormatVersion);
    }
    let revision =
        Uuid::from_slice(&encoded[9..25]).map_err(|_| StoreError::InvalidObjectFormat)?;
    let length = u64::from_be_bytes(
        encoded[25..33]
            .try_into()
            .map_err(|_| StoreError::InvalidObjectFormat)?,
    ) as usize;
    let bytes = encoded
        .get(HEADER_LEN..)
        .ok_or(StoreError::InvalidObjectFormat)?;
    if bytes.len() != length {
        return Err(StoreError::InvalidObjectFormat);
    }
    Ok(Object {
        bytes: bytes.to_vec(),
        version: Version::new(revision.to_string()),
    })
}
