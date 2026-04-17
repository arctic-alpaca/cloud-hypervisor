use std::collections::BTreeMap;
use std::fs::File;
use std::os::fd::AsRawFd;

use log::{debug, warn};
use option_parser::fd::FdDevice;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::ValidationError;
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
    #[serde(default, deserialize_with = "deserialize_fd_map_fds")]
    devices: BTreeMap<FdDevice, Vec<i32>>,
}

fn deserialize_fd_map_fds<'de, D>(
    d: D,
) -> std::result::Result<BTreeMap<FdDevice, Vec<i32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut invalidated_fds: BTreeMap<FdDevice, Vec<i32>> = BTreeMap::deserialize(d)?;
    invalidated_fds.values_mut().for_each(|fd_vec| {
        fd_vec.iter_mut().for_each(|fd| {
            // If the live-migration path is used properly, new FDs are passed as
            // SCM_RIGHTS message. So, we don't get them from the serialized JSON
            // anyway.
            debug!(
                "FDs in 'FdMap' won't be deserialized as they cannot cross process boundaries. Deserializing them as -1."
            );
            *fd = -1;
        });
    });
    Ok(invalidated_fds)
}

impl FdMap {
    pub fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, device: FdDevice, fd: i32, filter: FdFilter) -> bool {
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

    pub fn ingest_fds(&mut self, fds: Vec<File>) {
        for (device, fd) in fds.iter().zip(self.devices.values_mut()) {
            fd.push(device.as_raw_fd());
        }
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
