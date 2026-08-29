use std::fmt;

use objsds_store::Version;

use crate::BuildError;

/// An `objsds` operation error.
#[derive(Debug)]
pub enum Error<E> {
    /// The builder was not configured correctly.
    Configuration(BuildError),
    /// The object store could not complete the operation.
    Store(StoreError<E>),
    /// A document could not be encoded or decoded.
    Document(DocumentError),
    /// The requested data structure does not exist.
    NotFound,
    /// Creation raced with an existing object.
    AlreadyExists(AlreadyExistsError),
    /// A conditional replacement used a stale object version.
    Conflict(ConflictError),
    /// Persisted metadata is incompatible with the requested structure.
    Incompatible(CompatibilityError),
}

/// The object store could not complete an operation.
#[derive(Debug)]
pub struct StoreError<E> {
    pub source: E,
}

/// A document could not be encoded or decoded.
#[derive(Debug)]
pub enum DocumentError {
    /// A value supplied by the application could not be encoded.
    Serialize(serde_json::Error),
    /// The persisted document is malformed or cannot be decoded as the expected type.
    Deserialize(serde_json::Error),
    /// The decoded document violates a data-structure invariant.
    Corrupt { reason: String },
}

/// Creation found an object already present at the requested location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlreadyExistsError {
    pub observed: Version,
}

/// A conditional replacement did not observe the expected object version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictError {
    pub expected: Version,
    /// `None` means the object was deleted before the replacement.
    pub observed: Option<Version>,
}

/// Persisted metadata is incompatible with the requested structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibilityError {
    FormatVersion { expected: u32, observed: u32 },
    Kind { expected: String, observed: String },
    Schema { expected: String, observed: String },
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(error) => write!(formatter, "invalid configuration: {error}"),
            Self::Store(error) => write!(formatter, "object store error: {}", error.source),
            Self::Document(error) => error.fmt(formatter),
            Self::NotFound => formatter.write_str("data structure does not exist"),
            Self::AlreadyExists(_) => formatter.write_str("data structure already exists"),
            Self::Conflict(_) => formatter.write_str("object version conflict"),
            Self::Incompatible(error) => error.fmt(formatter),
        }
    }
}

impl<E: fmt::Display> fmt::Display for StoreError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl fmt::Display for AlreadyExistsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "object already exists at version {}",
            self.observed.as_str()
        )
    }
}

impl fmt::Display for ConflictError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.observed {
            Some(observed) => write!(
                formatter,
                "expected object version {}, observed {}",
                self.expected.as_str(),
                observed.as_str()
            ),
            None => write!(
                formatter,
                "expected object version {}, but the object was deleted",
                self.expected.as_str()
            ),
        }
    }
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(error) => write!(formatter, "could not encode JSON document: {error}"),
            Self::Deserialize(error) => {
                write!(formatter, "persisted JSON document is malformed: {error}")
            }
            Self::Corrupt { reason } => {
                write!(formatter, "persisted document is corrupt: {reason}")
            }
        }
    }
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("incompatible data structure: ")?;
        match self {
            Self::FormatVersion { expected, observed } => {
                write!(
                    formatter,
                    "expected format version {expected}, observed {observed}"
                )
            }
            Self::Kind { expected, observed } => {
                write!(formatter, "expected kind {expected}, observed {observed}")
            }
            Self::Schema { expected, observed } => {
                write!(formatter, "expected schema {expected}, observed {observed}")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::Store(error) => Some(&error.source),
            Self::Document(error) => Some(error),
            _ => None,
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for StoreError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl std::error::Error for AlreadyExistsError {}
impl std::error::Error for ConflictError {}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialize(error) | Self::Deserialize(error) => Some(error),
            Self::Corrupt { .. } => None,
        }
    }
}

impl std::error::Error for CompatibilityError {}
