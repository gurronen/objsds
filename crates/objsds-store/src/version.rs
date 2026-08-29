/// An opaque object version supplied by a store.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Version(String);

impl Version {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
