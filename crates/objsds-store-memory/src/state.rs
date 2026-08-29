use std::collections::BTreeMap;

use objsds_store::{Location, Object, Version};

#[derive(Debug, Default)]
pub(crate) struct State {
    pub(crate) objects: BTreeMap<Location, Object>,
    next_version: u64,
}

impl State {
    pub(crate) fn version(&mut self) -> Version {
        self.next_version += 1;
        Version::new(self.next_version.to_string())
    }
}
