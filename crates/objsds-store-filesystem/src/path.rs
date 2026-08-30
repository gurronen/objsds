use std::path::{Component, Path, PathBuf};

use objsds_store::Location;

use crate::StoreError;

pub(crate) fn object_path(root: &Path, location: &Location) -> Result<PathBuf, StoreError> {
    let relative = Path::new(location.as_str());
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(StoreError::InvalidLocation);
    }
    let path = root.join(relative);
    reject_existing_symlinks(root, &path)?;
    Ok(path)
}

pub(crate) fn reject_existing_symlinks(root: &Path, path: &Path) -> Result<(), StoreError> {
    let mut current = root.to_path_buf();
    for component in path
        .strip_prefix(root)
        .map_err(|_| StoreError::InvalidLocation)?
        .components()
    {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(StoreError::InvalidLocation);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub(crate) fn lock_path(root: &Path, location: &Location) -> PathBuf {
    let encoded = location
        .as_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    root.join(".objsds-locks").join(format!("{encoded}.lock"))
}
