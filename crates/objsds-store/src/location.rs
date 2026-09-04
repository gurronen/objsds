use std::fmt;

/// Returns whether `value` is one non-empty relative path segment.
///
/// Rejects the empty string, `.`, `..`, and any value containing `/`.
#[must_use]
pub fn is_path_segment(value: &str) -> bool {
    !value.is_empty() && value != "." && value != ".." && !value.contains('/')
}

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

    /// Builds `{namespace}/{kind}/{name}.json` after validating each segment.
    pub fn structure(namespace: &str, kind: &str, name: &str) -> Result<Self, InvalidLocation> {
        if ![namespace, kind, name].into_iter().all(is_path_segment) {
            return Err(InvalidLocation);
        }
        Self::new(format!("{namespace}/{kind}/{name}.json"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_segments_reject_empty_dot_and_slash() {
        assert!(is_path_segment("jobs"));
        assert!(!is_path_segment(""));
        assert!(!is_path_segment("."));
        assert!(!is_path_segment(".."));
        assert!(!is_path_segment("a/b"));
    }

    #[test]
    fn structure_joins_validated_segments() {
        let location = Location::structure("prod", "queues", "jobs")
            .expect("structure location should be valid");
        assert_eq!(location.as_str(), "prod/queues/jobs.json");
        assert!(Location::structure("prod", "queues", "..").is_err());
        assert!(Location::structure("", "queues", "jobs").is_err());
    }
}
