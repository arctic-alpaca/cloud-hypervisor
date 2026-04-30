use std::collections::BTreeMap;
use std::fs::File;
use std::os::fd::{AsRawFd, RawFd};

use log::warn;
pub(crate) use option_parser::fd::{FdDevice, SerializableFd};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::vm_config::VmConfig;

#[derive(Debug, Clone, Copy)]
pub enum FdFilter {
    Net,
}

impl FdFilter {
    pub fn filter(&self) -> fn(&FdDevice) -> bool {
        match self {
            FdFilter::Net => |device| matches!(device, FdDevice::Net { .. }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct FdMap {
    #[serde(default)]
    devices: BTreeMap<FdDevice, Vec<SerializableFd>>,
}

impl FdMap {
    pub fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub fn new_with_content(content: &[(FdDevice, Vec<RawFd>)]) -> Self {
        let mut fd_map = Self::new();
        content.iter().for_each(|(fd_device, raw_fds)| {
            let serializable_fds = raw_fds
                .iter()
                .map(|raw_fd| SerializableFd::new_valid(*raw_fd))
                .collect();
            fd_map.devices.insert(fd_device.clone(), serializable_fds);
        });
        fd_map
    }

    pub fn insert(&mut self, device: FdDevice, fd: SerializableFd, filter: FdFilter) -> bool {
        if filter.filter()(&device) {
            self.devices.entry(device).or_default().push(fd);
            true
        } else {
            false
        }
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn apply(mut self, vm_config: &mut VmConfig) -> Result<(), FdApplyError> {
        self.apply_net(vm_config)?;
        if !self.is_empty() {
            warn!("FDs not applied: {:?}", self.devices);
        }
        Ok(())
    }

    pub fn overwrite_fds_from_scm_rights(&mut self, mut fds: Vec<File>) {
        // TODO: proper error handling
        assert_eq!(
            self.devices.values().flatten().count(),
            fds.len(),
            "FD number does not match required number of FDs"
        );
        for (device, fd) in fds.drain(..).zip(self.devices.values_mut().flatten()) {
            *fd = SerializableFd::new_valid(device.as_raw_fd());
        }
    }

    pub fn extract_fds_for_scm_rights(&mut self) -> Vec<RawFd> {
        self.devices.values_mut().flatten().map(|fd| **fd).collect()
    }

    fn apply_net(&mut self, vm_config: &mut VmConfig) -> Result<(), FdApplyError> {
        let Some(net_configs) = vm_config.net.as_mut() else {
            return Ok(());
        };
        net_configs.iter_mut().try_for_each(|net_config| {
            // Devices without an id or FDs don't support FD reconstruction.
            if let Some(id) = net_config.id.as_ref()
                && let Some(outdated_fds) = net_config.fds.as_mut()
            {
                let mut updated_fds = self
                    .devices
                    .remove(&FdDevice::Net { id: id.to_owned() })
                    .ok_or(FdApplyError::Todo(
                        format!("FDs not found for device {id}",),
                    ))?;

                if outdated_fds.len() != updated_fds.len() {
                    return Err(FdApplyError::Todo(
                        "FD count mismatch between config and device".to_string(),
                    ));
                }

                outdated_fds.clear();
                outdated_fds.append(&mut updated_fds);
            }
            Ok(())
        })
    }
}

#[derive(Error, Debug, Eq, PartialEq)]
pub enum FdApplyError {
    #[error("Todo: {0}")]
    Todo(String),
}
