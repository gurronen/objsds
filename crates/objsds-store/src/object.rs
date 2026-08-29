use crate::Version;

/// Bytes read from one immutable object version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Object {
    pub bytes: Vec<u8>,
    pub version: Version,
}
