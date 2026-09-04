use std::fmt;
use std::sync::Arc;

use objsds_store::{Location, ObjectStore, is_path_segment};

use crate::{LogBuilder, MapBuilder};

/// Client sharing one object store and namespace.
///
/// Clones share the same store and create handles for independent single-object
/// structures. Constructing or cloning a client performs no object-store I/O.
#[derive(Debug)]
pub struct Objsds<S> {
    pub(crate) store: Arc<S>,
    pub(crate) namespace: String,
}

impl Objsds<()> {
    /// Starts a client builder with no store or namespace.
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
    /// Starts a builder for the named map.
    ///
    /// This performs no I/O; lifecycle methods on the returned builder do.
    #[must_use]
    pub fn map<V>(&self, name: impl Into<String>) -> MapBuilder<S, V> {
        MapBuilder::new(Arc::clone(&self.store), &self.namespace, name.into())
    }

    /// Starts a builder for the named log.
    ///
    /// This performs no I/O; lifecycle methods on the returned builder do.
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
    /// Selects the blocking object-store implementation shared by handles.
    #[must_use]
    pub fn store<T>(self, store: T) -> ObjsdsBuilder<T> {
        ObjsdsBuilder {
            store: Some(store),
            namespace: self.namespace,
        }
    }

    /// Sets the single path segment that prefixes every structure location.
    #[must_use]
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Validates configuration and constructs a client without performing I/O.
    pub fn build(self) -> Result<Objsds<S>, BuildError> {
        let namespace = self.namespace.ok_or(BuildError::MissingNamespace)?;
        if !is_path_segment(&namespace) {
            return Err(BuildError::InvalidNamespace);
        }
        Ok(Objsds {
            store: Arc::new(self.store.ok_or(BuildError::MissingStore)?),
            namespace,
        })
    }
}

pub(crate) fn location(namespace: &str, kind: &str, name: &str) -> Result<Location, BuildError> {
    if !is_path_segment(name) {
        return Err(BuildError::InvalidName);
    }
    Location::structure(namespace, kind, name).map_err(|_| BuildError::InvalidName)
}

/// Invalid client or structure configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildError {
    /// No object store was supplied to the client builder.
    MissingStore,
    /// No namespace was supplied to the client builder.
    MissingNamespace,
    /// The namespace is not one non-empty path segment.
    InvalidNamespace,
    /// No non-empty persistent schema identifier was supplied.
    MissingSchema,
    /// A structure name is not one non-empty path segment.
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
