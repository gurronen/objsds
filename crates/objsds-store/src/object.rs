use crate::Version;

/// Bytes read from one immutable object version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Object {
    /// Complete object contents from the same read as [`version`](Self::version).
    pub bytes: Vec<u8>,
    /// Opaque version identifying this snapshot.
    pub version: Version,
}
