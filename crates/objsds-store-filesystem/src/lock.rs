use std::fs::{File, OpenOptions};
use std::path::Path;

use fs4::fs_std::FileExt;

use crate::StoreError;

pub(crate) enum LockMode {
    Shared,
    Exclusive,
}

pub(crate) struct ObjectLock {
    _file: File,
}

impl ObjectLock {
    pub(crate) fn acquire(path: &Path, mode: LockMode) -> Result<Self, StoreError> {
        let parent = path.parent().ok_or(StoreError::InvalidLocation)?;
        std::fs::create_dir_all(parent)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        match mode {
            LockMode::Shared => file.lock_shared()?,
            LockMode::Exclusive => file.lock_exclusive()?,
        }
        Ok(Self { _file: file })
    }
}
