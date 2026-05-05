use std::collections::BTreeMap;
use std::fs::File;

use serde::{Deserialize, Serialize};
use serde_with::{MapPreventDuplicates, serde_as};

use crate::fd::SerializableFd;
use crate::fd_device::FdDevice;

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(transparent)]
pub struct FdMap {
    #[serde_as(as = "MapPreventDuplicates<_, _>")]
    fd_map: BTreeMap<FdDevice, Vec<SerializableFd>>,
}

impl FdMap {
    pub fn new() -> Self {
        Self {
            fd_map: BTreeMap::new(),
        }
    }

    pub fn new_with_entry(key: FdDevice, value: Vec<SerializableFd>) -> Self {
        Self {
            fd_map: BTreeMap::from([(key, value)]),
        }
    }

    pub fn merge(&mut self, other: FdMap) {
        other.fd_map.into_iter().for_each(|(device, mut fds)| {
            self.fd_map
                .entry(device)
                .and_modify(|entry| entry.append(&mut fds))
                .or_insert(fds);
        });
    }

    pub fn can_update(&self, other: &FdMap) -> bool {
        self.fd_map.iter().all(|(key, values)| {
            if let Some(other_values) = other.fd_map.get(key) {
                values.len() == other_values.len()
            } else {
                false
            }
        })
    }

    pub fn insert(&mut self, device: FdDevice, mut fd: Vec<SerializableFd>) {
        self.fd_map.entry(device).or_default().append(&mut fd);
    }

    pub fn remove(&mut self, device: &FdDevice) -> Option<Vec<SerializableFd>> {
        self.fd_map.remove(device)
    }

    pub fn is_empty(&self) -> bool {
        self.fd_map.is_empty()
    }

    pub fn is_valid(&self) -> bool {
        self.fd_map.values().flatten().all(|fd| fd.is_active())
    }

    pub fn update_fds(&mut self, mut fds: Vec<File>) {
        // TODO(fd): proper error handling
        assert_eq!(
            self.fd_map.values().flatten().count(),
            fds.len(),
            "FD number does not match required number of FDs"
        );
        for (device, fd) in fds.drain(..).zip(self.fd_map.values_mut().flatten()) {
            *fd = SerializableFd::new_active(device.into());
        }
    }

    pub fn extract_fds(&mut self) -> Vec<SerializableFd> {
        self.fd_map
            .values_mut()
            .flatten()
            .map(|fd| fd.clone())
            .collect()
    }
}

#[cfg(test)]
mod unit_tests {
    use crate::{FdDevice, FdMap, SerializableFd};

    #[test]
    fn test() {
        let fd_map = FdMap::new_with_entry(
            FdDevice::Net {
                id: "10".to_owned(),
            },
            vec![SerializableFd::new_serialized(1)],
        );
        let fd_map_json = serde_json::to_string(&fd_map).unwrap();

        assert_eq!(fd_map_json, r#"{"net(10)":[1]}"#);
    }
}
