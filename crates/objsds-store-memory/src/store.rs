use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use objsds_store::{CreateError, Location, Object, ObjectStore, ReplaceError, Version};

use crate::state::State;

#[derive(Clone, Debug, Default)]
pub struct MemoryStore {
    inner: Arc<Mutex<State>>,
}

impl ObjectStore for MemoryStore {
    type Error = Infallible;

    fn get(&self, location: &Location) -> Result<Option<Object>, Self::Error> {
        let state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        Ok(state.objects.get(location).cloned())
    }

    fn create(
        &self,
        location: &Location,
        bytes: &[u8],
    ) -> Result<Version, CreateError<Self::Error>> {
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(object) = state.objects.get(location) {
            return Err(CreateError::AlreadyExists {
                observed: object.version.clone(),
            });
        }

        let version = state.version();
        state.objects.insert(
            location.clone(),
            Object {
                bytes: bytes.to_vec(),
                version: version.clone(),
            },
        );
        Ok(version)
    }

    fn replace(
        &self,
        location: &Location,
        expected: &Version,
        bytes: &[u8],
    ) -> Result<Version, ReplaceError<Self::Error>> {
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        match state.objects.get(location) {
            Some(object) if &object.version == expected => {}
            object => {
                return Err(ReplaceError::Conflict {
                    observed: object.map(|object| object.version.clone()),
                });
            }
        }

        let version = state.version();
        state.objects.insert(
            location.clone(),
            Object {
                bytes: bytes.to_vec(),
                version: version.clone(),
            },
        );
        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_stale_replacement() {
        let store = MemoryStore::default();
        let location = Location::new("maps/users.json").expect("location should be valid");
        let first = store
            .create(&location, b"first")
            .expect("initial create should succeed");
        let second = store
            .replace(&location, &first, b"second")
            .expect("current replacement should succeed");

        let error = store
            .replace(&location, &first, b"stale")
            .expect_err("stale replacement should conflict");
        assert!(matches!(
            error,
            ReplaceError::Conflict {
                observed: Some(observed)
            } if observed == second
        ));
    }
}
