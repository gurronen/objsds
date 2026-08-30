use std::fmt;
use std::io;

/// Invalid filesystem adapter configuration.
#[derive(Debug)]
pub enum BuildError {
    /// No root directory was configured.
    MissingRoot,
    /// The configured root could not be prepared.
    Io(io::Error),
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRoot => formatter.write_str("missing filesystem root"),
            Self::Io(error) => write!(formatter, "could not prepare filesystem root: {error}"),
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingRoot => None,
            Self::Io(error) => Some(error),
        }
    }
}

/// A filesystem object operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// The location cannot safely be represented below the configured root.
    InvalidLocation,
    /// Filesystem I/O failed.
    Io(io::Error),
    /// A managed file did not contain a valid adapter envelope.
    InvalidObjectFormat,
    /// A managed file uses an unsupported adapter envelope version.
    UnsupportedFormatVersion,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLocation => formatter.write_str("invalid filesystem object location"),
            Self::Io(error) => write!(formatter, "filesystem operation failed: {error}"),
            Self::InvalidObjectFormat => formatter.write_str("invalid filesystem object format"),
            Self::UnsupportedFormatVersion => {
                formatter.write_str("unsupported filesystem object format version")
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
