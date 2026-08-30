use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use objsds_store::{CreateError, Location, Object, ObjectStore, ReplaceError, Version};
use uuid::Uuid;

use crate::format::{decode, encode};
use crate::lock::{LockMode, ObjectLock};
use crate::path::{lock_path, object_path, reject_existing_symlinks};
use crate::{BuildError, StoreError};

#[derive(Clone)]
/// Configured blocking filesystem object store.
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

    fn read_path(path: &Path) -> Result<Option<Object>, StoreError> {
        match std::fs::read(path) {
            Ok(bytes) => decode(&bytes).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn prepare_path(&self, location: &Location) -> Result<PathBuf, StoreError> {
        let path = object_path(&self.root, location)?;
        let parent = path.parent().ok_or(StoreError::InvalidLocation)?;
        std::fs::create_dir_all(parent)?;
        reject_existing_symlinks(&self.root, &path)?;
        Ok(path)
    }

    fn publish(path: &Path, encoded: &[u8]) -> Result<(), StoreError> {
        let parent = path.parent().ok_or(StoreError::InvalidLocation)?;
        let temporary = parent.join(format!(".objsds-{}.tmp", Uuid::now_v7()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(encoded)?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
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
        if !root.is_dir() {
            return Err(BuildError::Io(std::io::Error::other(
                "filesystem root is not a directory",
            )));
        }
        std::fs::create_dir_all(root.join(".objsds-locks")).map_err(BuildError::Io)?;
        Ok(FilesystemStore { root })
    }
}

impl ObjectStore for FilesystemStore {
    type Error = StoreError;
    fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error> {
        let path = object_path(&self.root, location)?;
        let _lock = ObjectLock::acquire(&lock_path(&self.root, location), LockMode::Shared)?;
        Self::read_path(&path)
    }
    fn create(
        &self,
        location: &Location,
        bytes: &[u8],
    ) -> Result<Version, CreateError<Self::Error>> {
        let path = self.prepare_path(location).map_err(CreateError::Store)?;
        let _lock = ObjectLock::acquire(&lock_path(&self.root, location), LockMode::Exclusive)
            .map_err(CreateError::Store)?;
        if let Some(object) = Self::read_path(&path).map_err(CreateError::Store)? {
            return Err(CreateError::AlreadyExists {
                observed: object.version,
            });
        }
        let (version, encoded) = encode(bytes);
        Self::publish(&path, &encoded).map_err(CreateError::Store)?;
        Ok(version)
    }
    fn replace(
        &self,
        location: &Location,
        expected: &Version,
        bytes: &[u8],
    ) -> Result<Version, ReplaceError<Self::Error>> {
        let path = self.prepare_path(location).map_err(ReplaceError::Store)?;
        let _lock = ObjectLock::acquire(&lock_path(&self.root, location), LockMode::Exclusive)
            .map_err(ReplaceError::Store)?;
        let current = Self::read_path(&path).map_err(ReplaceError::Store)?;
        if current.as_ref().map(|object| &object.version) != Some(expected) {
            return Err(ReplaceError::Conflict {
                observed: current.map(|object| object.version),
            });
        }
        let (version, encoded) = encode(bytes);
        Self::publish(&path, &encoded).map_err(ReplaceError::Store)?;
        Ok(version)
    }
}
