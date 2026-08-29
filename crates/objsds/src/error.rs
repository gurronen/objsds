use std::fmt;

use objsds_store::Version;

/// An `objsds` operation error.
#[derive(Debug)]
pub enum Error<E> {
    Store(E),
    Json(serde_json::Error),
    NotFound,
    AlreadyExists { observed: Version },
    Conflict { observed: Option<Version> },
    Incompatible { expected: String, observed: String },
}

pub type Result<T, E> = std::result::Result<T, Error<E>>;

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(formatter, "object store error: {error}"),
            Self::Json(error) => write!(formatter, "invalid stored JSON: {error}"),
            Self::NotFound => formatter.write_str("data structure does not exist"),
            Self::AlreadyExists { .. } => formatter.write_str("data structure already exists"),
            Self::Conflict { .. } => formatter.write_str("object version conflict"),
            Self::Incompatible { expected, observed } => {
                write!(
                    formatter,
                    "incompatible data structure: expected {expected}, observed {observed}"
                )
            }
        }
    }
}

impl<E> From<serde_json::Error> for Error<E> {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}
