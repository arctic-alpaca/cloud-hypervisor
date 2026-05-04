use std::collections::BTreeMap;
use std::fs::File;

use serde::{Deserialize, Serialize};
use serializable_fd::{FdDevice, SerializableFd};
use thiserror::Error;

use crate::vm_config::VmConfig;

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
    pub fn new_with_content(content: &[(FdDevice, Vec<std::os::fd::RawFd>)]) -> Self {
        let mut fd_map = Self::new();
        content.iter().for_each(|(fd_device, raw_fds)| {
            let serializable_fds = raw_fds
                .iter()
                .map(|raw_fd|
                    // SAFETY: TODO(fd)
                    unsafe{
                    SerializableFd::new_valid_from_raw(*raw_fd)
                })
                .collect();
            fd_map.devices.insert(fd_device.clone(), serializable_fds);
        });
        fd_map
    }

    pub fn insert(&mut self, device: FdDevice, fd: SerializableFd) {
        self.devices.entry(device).or_default().push(fd);
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn update_fds(&mut self, mut fds: Vec<File>) {
        // TODO(fd): proper error handling
        assert_eq!(
            self.devices.values().flatten().count(),
            fds.len(),
            "FD number does not match required number of FDs"
        );
        for (device, fd) in fds.drain(..).zip(self.devices.values_mut().flatten()) {
            *fd = SerializableFd::new_valid(device.into());
        }
    }

    pub fn extract_fds(&mut self) -> Vec<SerializableFd> {
        self.devices
            .values_mut()
            .flatten()
            .filter_map(|fd| fd.extract_fd())
            .collect()
    }

    pub fn apply(self, vm_config: &mut VmConfig) -> Result<(), FdApplyError> {
        for (device, fds) in self.devices.into_iter() {
            match device {
                FdDevice::Net { ref id } => Self::apply_net(id, fds, vm_config)?,
            }
        }

        Ok(())
    }

    pub fn validate(&self, vm_config: &VmConfig) -> Result<(), FdApplyError> {
        for (device, fds) in self.devices.iter() {
            match &device {
                FdDevice::Net { id } => Self::validate_net(id, fds, vm_config)?,
            }
        }

        Ok(())
    }
    fn validate_net(
        net_device_id: &String,
        fds: &[SerializableFd],
        vm_config: &VmConfig,
    ) -> Result<(), FdApplyError> {
        let Some(net_configs) = vm_config.net.as_ref() else {
            return Err(FdApplyError::Todo(
                "VM config is missing net devices".to_owned(),
            ));
        };
        let Some(net_config) = net_configs
            .iter()
            .find(|config| config.id.as_ref() == Some(net_device_id))
        else {
            return Err(FdApplyError::Todo(format!(
                "could not find net device with id {net_device_id}"
            )));
        };

        let Some(outdated_net_fds) = net_config.fds.as_ref() else {
            return Err(FdApplyError::Todo(format!(
                "cannot restore FDs for {net_device_id}, device does not use FDs"
            )));
        };

        if outdated_net_fds.len() != fds.len() {
            return Err(FdApplyError::Todo(
                "FD count mismatch between config and device".to_string(),
            ));
        }

        Ok(())
    }

    fn apply_net(
        net_device_id: &String,
        mut fds: Vec<SerializableFd>,
        vm_config: &mut VmConfig,
    ) -> Result<(), FdApplyError> {
        let Some(net_configs) = vm_config.net.as_mut() else {
            return Err(FdApplyError::Todo(
                "VM config is missing net devices".to_owned(),
            ));
        };
        let Some(net_config) = net_configs
            .iter_mut()
            .find(|config| config.id.as_ref() == Some(net_device_id))
        else {
            return Err(FdApplyError::Todo(format!(
                "could not find net device with id {net_device_id}"
            )));
        };

        let Some(outdated_net_fds) = net_config.fds.as_mut() else {
            return Err(FdApplyError::Todo(format!(
                "cannot restore FDs for {net_device_id}, device does not use FDs"
            )));
        };

        if outdated_net_fds.len() != fds.len() {
            return Err(FdApplyError::Todo(
                "FD count mismatch between config and device".to_string(),
            ));
        }

        outdated_net_fds.clear();
        outdated_net_fds.append(&mut fds);

        Ok(())
    }
}

#[derive(Error, Debug, Eq, PartialEq)]
pub enum FdApplyError {
    #[error("Todo: {0}")]
    Todo(String),
}
