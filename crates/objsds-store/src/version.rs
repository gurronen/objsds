/// An opaque object version supplied by a store.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Version(String);

impl Version {
    /// Wraps a backend-supplied opaque version token.
    ///
    /// Equality is meaningful only for versions from the same object location
    /// and store. The token need not be numeric or ordered.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the backend token exactly as supplied at construction.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
