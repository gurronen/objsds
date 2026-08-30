use std::fmt;

/// A provider-independent object location.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Location(String);

impl Location {
    /// Creates a relative, non-empty, normalized object location.
    ///
    /// Rejects leading slashes and repeated separators. Other provider-specific
    /// restrictions remain the responsibility of the adapter.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidLocation> {
        let value = value.into();
        if value.is_empty() || value.starts_with('/') || value.contains("//") {
            return Err(InvalidLocation);
        }
        Ok(Self(value))
    }

    /// Returns the provider-independent relative location.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// An invalid object location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidLocation;

impl fmt::Display for InvalidLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("object location must be relative, non-empty, and normalized")
    }
}

impl std::error::Error for InvalidLocation {}
