use std::fmt;
use std::sync::Arc;

use objsds_store::ObjectStore;

use crate::{LogBuilder, MapBuilder};

/// Client sharing one object store and namespace.
#[derive(Debug)]
pub struct Objsds<S> {
    pub(crate) store: Arc<S>,
    pub(crate) namespace: String,
}

impl Objsds<()> {
    #[must_use]
    pub fn builder() -> ObjsdsBuilder<()> {
        ObjsdsBuilder::default()
    }
}

impl<S> Clone for Objsds<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            namespace: self.namespace.clone(),
        }
    }
}

impl<S: ObjectStore> Objsds<S> {
    #[must_use]
    pub fn map<V>(&self, name: impl Into<String>) -> MapBuilder<S, V> {
        MapBuilder::new(Arc::clone(&self.store), &self.namespace, name.into())
    }

    #[must_use]
    pub fn log<V>(&self, name: impl Into<String>) -> LogBuilder<S, V> {
        LogBuilder::new(Arc::clone(&self.store), &self.namespace, name.into())
    }
}

/// Builder for [`Objsds`].
#[derive(Debug)]
pub struct ObjsdsBuilder<S> {
    store: Option<S>,
    namespace: Option<String>,
}

impl Default for ObjsdsBuilder<()> {
    fn default() -> Self {
        Self {
            store: None,
            namespace: None,
        }
    }
}

impl<S> ObjsdsBuilder<S> {
    #[must_use]
    pub fn store<T>(self, store: T) -> ObjsdsBuilder<T> {
        ObjsdsBuilder {
            store: Some(store),
            namespace: self.namespace,
        }
    }

    #[must_use]
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn build(self) -> Result<Objsds<S>, BuildError> {
        let namespace = self.namespace.ok_or(BuildError::MissingNamespace)?;
        validate_segment(&namespace).map_err(|()| BuildError::InvalidNamespace)?;
        Ok(Objsds {
            store: Arc::new(self.store.ok_or(BuildError::MissingStore)?),
            namespace,
        })
    }
}

pub(crate) fn location(
    namespace: &str,
    kind: &str,
    name: &str,
) -> Result<objsds_store::Location, BuildError> {
    validate_segment(name).map_err(|()| BuildError::InvalidName)?;
    objsds_store::Location::new(format!("{namespace}/{kind}/{name}.json"))
        .map_err(|_| BuildError::InvalidName)
}

fn validate_segment(value: &str) -> Result<(), ()> {
    if value.is_empty() || value == "." || value == ".." || value.contains('/') {
        Err(())
    } else {
        Ok(())
    }
}

/// Invalid client or structure configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildError {
    MissingStore,
    MissingNamespace,
    InvalidNamespace,
    MissingSchema,
    InvalidName,
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::MissingStore => "missing object store",
            Self::MissingNamespace => "missing namespace",
            Self::InvalidNamespace => "namespace must be one non-empty path segment",
            Self::MissingSchema => "missing schema identifier",
            Self::InvalidName => "structure name must be one non-empty path segment",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for BuildError {}
