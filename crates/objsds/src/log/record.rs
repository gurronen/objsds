use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque, sortable identifier for a Log record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LogId(pub(super) Uuid);

impl LogId {
    pub(super) fn now() -> Self {
        Self(Uuid::now_v7())
    }
}

impl std::fmt::Display for LogId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One immutable Log record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Record<V> {
    pub id: LogId,
    pub value: V,
}
