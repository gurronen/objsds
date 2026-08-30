use std::fmt;
use std::path::{Path, PathBuf};

use crate::BuildError;

/// Configured blocking filesystem object store.
#[derive(Clone)]
pub struct FilesystemStore {
    pub(crate) root: PathBuf,
}

impl FilesystemStore {
    /// Starts a filesystem store builder with no root directory.
    #[must_use]
    pub fn builder() -> FilesystemStoreBuilder {
        FilesystemStoreBuilder::default()
    }

    /// Returns the canonical root containing managed data.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl fmt::Debug for FilesystemStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FilesystemStore")
            .field("root", &self.root)
            .finish()
    }
}

/// Builder for [`FilesystemStore`].
#[derive(Clone, Debug, Default)]
pub struct FilesystemStoreBuilder {
    root: Option<PathBuf>,
}

impl FilesystemStoreBuilder {
    /// Sets the directory beneath which objects and adapter metadata are kept.
    #[must_use]
    pub fn root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = Some(root.into());
        self
    }

    /// Creates and canonicalizes the configured root.
    pub fn build(self) -> Result<FilesystemStore, BuildError> {
        let root = self.root.ok_or(BuildError::MissingRoot)?;
        std::fs::create_dir_all(&root).map_err(BuildError::Io)?;
        let root = root.canonicalize().map_err(BuildError::Io)?;
        Ok(FilesystemStore { root })
    }
}
