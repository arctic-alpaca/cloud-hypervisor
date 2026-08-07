// Copyright © 2026 Cyberus Technology GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs::File;
use std::os::fd::{IntoRawFd, RawFd};
use std::str::FromStr;

use option_parser::Tuple;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::RestoredNetConfig;
use crate::vm_config::{NetConfig, VmConfig};

/// TODO(fd)
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Device {
    Net { id: String },
    Todo,
}

impl Device {
    pub fn new_net(id: String) -> Self {
        Self::Net { id }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ParseDeviceError {
    InvalidValue(String),
    EmptyIdent(String),
}

impl FromStr for Device {
    type Err = ParseDeviceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ident, rest) = s.split_once("(").unwrap_or((s, ""));

        match ident {
            "net" => {
                if let Some((id, "")) = rest.split_once(")") {
                    if id.is_empty() {
                        Err(ParseDeviceError::EmptyIdent(s.to_owned()))
                    } else {
                        Ok(Self::new_net(id.to_owned()))
                    }
                } else {
                    Err(ParseDeviceError::InvalidValue(s.to_owned()))
                }
            }
            _ => Err(ParseDeviceError::InvalidValue(s.to_owned())),
        }
    }
}

/// TODO(fd)
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFds {
    device: Device,
    expected_fds: usize,
    #[serde(skip)]
    received_fds: Vec<RawFd>,
}

impl DeviceFds {
    /// TODO(fd)
    pub fn update_from_scm_rights(
        &mut self,
        files: &mut Vec<File>,
    ) -> Result<(), IngestScmRightsError> {
        if self.expected_fds <= files.len() {
            self.received_fds = files
                .drain(..self.expected_fds)
                .map(IntoRawFd::into_raw_fd)
                .collect();
            Ok(())
        } else {
            Err(IngestScmRightsError::TooLittleFds)
        }
    }

    /// TODO(fd)
    pub fn fds(&mut self) -> Vec<RawFd> {
        mem::take(&mut self.received_fds)
    }

    pub fn new(device: Device, files: Vec<File>) -> Self {
        Self {
            device,
            expected_fds: files.len(),
            received_fds: files.into_iter().map(IntoRawFd::into_raw_fd).collect(),
        }
    }

    pub fn new_raw(device: Device, raw_fds: Vec<RawFd>) -> Self {
        Self {
            device,
            expected_fds: raw_fds.len(),
            received_fds: raw_fds,
        }
    }
}

impl Clone for DeviceFds {
    fn clone(&self) -> Self {
        Self {
            device: self.device.clone(),
            expected_fds: self.expected_fds,
            received_fds: self
                .received_fds
                .iter()
                .map(|fd| {
                    // SAFETY: `dup` doesn't modify the parameter and the result is checked.
                    let duplicated_fd = unsafe { libc::dup(*fd) };
                    if duplicated_fd == -1 && *fd != -1 {
                        panic!("Failed to duplicate file descriptor");
                    }
                    duplicated_fd
                })
                .collect(),
        }
    }
}

impl From<RestoredNetConfig> for DeviceFds {
    fn from(value: RestoredNetConfig) -> Self {
        DeviceFds {
            device: Device::new_net(value.id),
            expected_fds: value.num_fds,
            // `RestoredNetConfig` may contain valid file descriptors if passed via CLI.
            received_fds: value
                .fds
                .map(|fds| fds.iter().filter(|fd| **fd != -1).copied().collect())
                .unwrap_or_default(),
        }
    }
}

/// TODO(fd)
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExternalFds {
    #[serde(default)]
    external_fds: Vec<DeviceFds>,
}

impl From<Vec<DeviceFds>> for ExternalFds {
    fn from(value: Vec<DeviceFds>) -> Self {
        Self {
            external_fds: value,
        }
    }
}

/// TODO(fd)
#[derive(Error, Debug, Eq, PartialEq)]
pub enum IngestScmRightsError {
    #[error("Less file descriptors provided than expected")]
    TooLittleFds,
    #[error("More file descriptors provided than expected")]
    TooManyFds,
}

impl ExternalFds {
    /// TODO(fd)
    pub fn take_fds(&mut self, device: &Device) -> Option<DeviceFds> {
        let position = self
            .external_fds
            .iter()
            .position(|device_fds| &device_fds.device == device)?;
        Some(self.external_fds.swap_remove(position))
    }

    pub fn new_for_single(device: Device, files: Vec<File>) -> Self {
        Self {
            external_fds: vec![DeviceFds::new(device, files)],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.external_fds.is_empty()
    }

    pub fn import_restored_net_config(&mut self, restored_net_configs: Vec<RestoredNetConfig>) {
        self.external_fds
            .splice(0..0, restored_net_configs.into_iter().map(Into::into));
    }

    pub fn fds(&mut self) -> Vec<RawFd> {
        self.external_fds
            .iter_mut()
            .flat_map(DeviceFds::fds)
            .collect()
    }
}

impl From<Tuple<Device, Vec<u64>>> for ExternalFds {
    fn from(value: Tuple<Device, Vec<u64>>) -> Self {
        Self {
            external_fds: value
                .0
                .into_iter()
                .map(|(device, fds)| {
                    DeviceFds::new_raw(device, fds.iter().map(|fd| *fd as RawFd).collect())
                })
                .collect(),
        }
    }
}

/// TODO(fd)
#[derive(Error, Debug, Eq, PartialEq)]
pub enum FdUpdateError {
    #[error(
        "Mismatch between expected and actual file descriptor number for device \"{device:?}\": actual: {actual}, expected: {expected}"
    )]
    FdAmountMismatch {
        device: Device,
        expected: usize,
        actual: usize,
    },
    #[error("More file descriptors provided than expected for: {0:?}")]
    TooManyFds(Device),
    #[error("Device didn't expect file descriptors: {0:?}")]
    UnexpectedFds(Device),
    #[error("Device without id expected file descriptors")]
    MissingId,
    #[error("Missing file descriptors for device: {0:?}")]
    MissingFds(Device),
    #[error("File descriptors for the following devices were unused: {0:?}")]
    SuperfluousFds(Vec<Device>),
    #[error("Failed to ingest SCM_RIGHTS: {0}")]
    IngestScmRights(#[from] IngestScmRightsError),
}

pub trait IngestScmRights {
    fn ingest_fds(&mut self, fds: Vec<File>) -> Result<(), FdUpdateError>;
}

impl IngestScmRights for NetConfig {
    fn ingest_fds(&mut self, fds: Vec<File>) -> Result<(), FdUpdateError> {
        let fds: Vec<RawFd> = fds.into_iter().map(IntoRawFd::into_raw_fd).collect();
        if !fds.is_empty() {
            self.fds = Some(fds);
        }
        Ok(())
    }
}

impl IngestScmRights for ExternalFds {
    fn ingest_fds(&mut self, mut fds: Vec<File>) -> Result<(), FdUpdateError> {
        self.external_fds
            .iter_mut()
            .try_for_each(|device_fds| device_fds.update_from_scm_rights(&mut fds))?;
        if fds.is_empty() {
            Ok(())
        } else {
            Err(FdUpdateError::IngestScmRights(
                IngestScmRightsError::TooManyFds,
            ))
        }
    }
}

pub trait UpdateFds {
    /// TODO(fd)
    fn update_fds(&mut self, fds: &mut ExternalFds) -> Result<(), FdUpdateError>;
    /// TODO(fd)
    fn consume_fds(&mut self, mut fds: ExternalFds) -> Result<(), FdUpdateError> {
        self.update_fds(&mut fds)?;
        if fds.is_empty() {
            Ok(())
        } else {
            Err(FdUpdateError::SuperfluousFds(
                fds.external_fds
                    .iter()
                    .map(|device_fds| device_fds.device.clone())
                    .collect(),
            ))
        }
    }
}

impl UpdateFds for NetConfig {
    fn update_fds(&mut self, fds: &mut ExternalFds) -> Result<(), FdUpdateError> {
        let Some(id) = &self.pci_common.id else {
            return Err(FdUpdateError::MissingId);
        };

        let Some(net_fds) = &mut self.fds else {
            return if fds.take_fds(&Device::new_net(id.clone())).is_some() {
                Err(FdUpdateError::UnexpectedFds(Device::new_net(id.clone())))
            } else {
                Ok(())
            };
        };

        let Some(mut received_fds) = fds.take_fds(&Device::new_net(id.clone())) else {
            return Err(FdUpdateError::MissingFds(Device::new_net(id.clone())));
        };

        let received_fds = received_fds.fds();

        if net_fds.len() != received_fds.len() {
            return Err(FdUpdateError::FdAmountMismatch {
                device: Device::new_net(id.clone()),
                expected: net_fds.len(),
                actual: received_fds.len(),
            });
        }

        *net_fds = received_fds;

        Ok(())
    }
}

impl UpdateFds for VmConfig {
    fn update_fds(&mut self, fds: &mut ExternalFds) -> Result<(), FdUpdateError> {
        self.net.iter_mut().try_for_each(|net_configs| {
            net_configs
                .iter_mut()
                .try_for_each(|net_config| net_config.update_fds(fds))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use option_parser::{OptionParser, Tuple};
    use serde::{Deserialize, Serialize};

    use crate::external_fds::{Device, DeviceFds, ExternalFds, ParseDeviceError};

    #[test]
    fn test_parse_device() {
        assert_eq!(
            Device::Net {
                id: "foo".to_owned()
            },
            Device::from_str("net(foo)").unwrap()
        );

        assert_eq!(
            ParseDeviceError::EmptyIdent("net()".to_owned()),
            Device::from_str("net()").unwrap_err()
        );

        assert_eq!(
            ParseDeviceError::InvalidValue("net((".to_owned()),
            Device::from_str("net((").unwrap_err()
        );

        assert_eq!(
            ParseDeviceError::InvalidValue("net".to_owned()),
            Device::from_str("net").unwrap_err()
        );
    }

    #[test]
    fn parse_external_fds() {
        let mut parser = OptionParser::new();
        parser.add("fds");
        parser.parse("fds=[net(1)@[1,2],net(2)@[3,4]]").unwrap();

        let external_fds: ExternalFds = parser
            .convert::<Tuple<Device, Vec<u64>>>("fds")
            .unwrap()
            .unwrap()
            .into();

        assert_eq!(
            external_fds,
            ExternalFds {
                external_fds: vec![
                    DeviceFds::new_raw(Device::new_net("1".to_owned()), vec![1, 2]),
                    DeviceFds::new_raw(Device::new_net("2".to_owned()), vec![3, 4]),
                ]
            }
        );
    }

    #[test]
    fn parse_external_fds_json() {
        #[derive(Serialize, Deserialize)]
        struct Dummy {
            #[serde(default, flatten)]
            external_fds: ExternalFds,
        }

        let serialized = serde_json::to_string(&Dummy {
            external_fds: ExternalFds {
                external_fds: vec![
                    DeviceFds::new_raw(Device::new_net("1".to_owned()), vec![1, 2]),
                    DeviceFds::new_raw(Device::new_net("2".to_owned()), vec![3, 4]),
                ],
            },
        })
        .unwrap();

        assert_eq!(
            serialized,
            r#"{"external_fds":[{"device":{"Net":{"id":"1"}},"expected_fds":2},{"device":{"Net":{"id":"2"}},"expected_fds":2}]}"#
        );

        let external_fds: Dummy = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            external_fds.external_fds,
            ExternalFds {
                external_fds: vec![
                    DeviceFds {
                        device: Device::new_net("1".to_owned()),
                        expected_fds: 2,
                        received_fds: vec![],
                    },
                    DeviceFds {
                        device: Device::new_net("2".to_owned()),
                        expected_fds: 2,
                        received_fds: vec![],
                    },
                ]
            }
        );
    }
}
