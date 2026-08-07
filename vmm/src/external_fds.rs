// Copyright © 2026 Cyberus Technology GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::BTreeSet;
use std::fs::File;
use std::mem;
use std::os::fd::{IntoRawFd, RawFd};
use std::str::FromStr;

use option_parser::{Tuple, TupleList};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::api::VmReceiveMigrationData;
use crate::config::{RestoreConfig, RestoredNetConfig, RestoredVfioConfig};
use crate::vm_config::{DeviceConfig, NetConfig, PlatformConfig, VmConfig};

/// Defines which operation caused the external file descriptor handling.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialOrd, PartialEq)]
pub enum ExternalFdOperation {
    Restore,
    ReceiveMigration,
    VmCreate,
}

/// A resource that can be backed by one or more file descriptors.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ExternalFdTarget {
    Net { id: String },
    Vfio { id: String },
    Iommu,
}

/// Errors during parsing [`ExternalFdTarget`].
#[derive(Debug, Eq, PartialEq)]
pub enum ParseExternalFdTargetError {
    InvalidValue(String),
    EmptyIdent(String),
}

impl FromStr for ExternalFdTarget {
    type Err = ParseExternalFdTargetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ident, rest) = s.split_once("(").unwrap_or((s, ""));

        fn parse_id(
            input: &str,
            constructor: fn(String) -> ExternalFdTarget,
            original: &str,
        ) -> Result<ExternalFdTarget, <ExternalFdTarget as FromStr>::Err> {
            if let Some((id, "")) = input.split_once(")") {
                if id.is_empty() {
                    Err(ParseExternalFdTargetError::EmptyIdent(original.to_owned()))
                } else {
                    Ok(constructor(id.to_owned()))
                }
            } else {
                Err(ParseExternalFdTargetError::InvalidValue(
                    original.to_owned(),
                ))
            }
        }

        match ident {
            "net" => parse_id(rest, |id| ExternalFdTarget::Net { id }, s),
            "vfio" => parse_id(rest, |id| ExternalFdTarget::Vfio { id }, s),
            "iommu" => {
                if rest.is_empty() {
                    Ok(ExternalFdTarget::Iommu)
                } else {
                    Err(ParseExternalFdTargetError::InvalidValue(s.to_owned()))
                }
            }
            _ => Err(ParseExternalFdTargetError::InvalidValue(s.to_owned())),
        }
    }
}

/// Metadata and file descriptors for one [`ExternalFdTarget`].
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExternalFdsEntry {
    target: ExternalFdTarget,
    expected_fds: usize,
    #[serde(skip)]
    received_fds: Vec<RawFd>,
}

// In tests, we're not using actual file descriptors, so closing them may close unrelated file descriptors.
#[cfg(not(test))]
impl Drop for ExternalFdsEntry {
    fn drop(&mut self) {
        self.received_fds
            .iter()
            .filter(|fd| **fd != -1)
            .for_each(|fd| {
                // SAFETY: Since this is a `RawFd`, there aren't any safety requirements to uphold.
                unsafe { libc::close(*fd) };
            });
    }
}

impl ExternalFdsEntry {
    /// Updates all file descriptors from the provided `files` list.
    pub(crate) fn update_from_scm_rights(
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

    /// Takes all file descriptors out of `Self`.
    pub(crate) fn take_fds(&mut self) -> Vec<RawFd> {
        mem::take(&mut self.received_fds)
    }

    /// Returns a reference to the contained file descriptors.
    pub(crate) fn fds(&self) -> &[RawFd] {
        &self.received_fds
    }

    /// Creates a new entry.
    pub(crate) fn new<R>(target: ExternalFdTarget, files: Vec<R>) -> Self
    where
        R: IntoRawFd,
    {
        Self {
            target,
            expected_fds: files.len(),
            received_fds: files.into_iter().map(IntoRawFd::into_raw_fd).collect(),
        }
    }
}

impl Clone for ExternalFdsEntry {
    fn clone(&self) -> Self {
        // In tests, we're not using actual file descriptors, so duplicating can either fail or
        // duplicate an existing, unrelated file descriptor.
        let received_fds = if cfg!(test) {
            self.received_fds.clone()
        } else {
            self.received_fds
                .iter()
                .map(|fd| {
                    // SAFETY: `dup` doesn't modify the parameter and the result is checked.
                    let duplicated_fd = unsafe { libc::dup(*fd) };
                    if duplicated_fd == -1 && *fd != -1 {
                        panic!("Failed to duplicate file descriptor");
                    }
                    duplicated_fd
                })
                .collect()
        };

        Self {
            target: self.target.clone(),
            expected_fds: self.expected_fds,
            received_fds,
        }
    }
}

// TODO(fd): Remove after `RestoredNetConfig` is deprecated and removed.
impl From<RestoredNetConfig> for ExternalFdsEntry {
    fn from(value: RestoredNetConfig) -> Self {
        ExternalFdsEntry {
            target: ExternalFdTarget::Net { id: value.id },
            expected_fds: value.num_fds,
            // `RestoredNetConfig` may contain valid file descriptors if passed via CLI.
            received_fds: value
                .fds
                .map(|fds| fds.iter().filter(|fd| **fd != -1).copied().collect())
                .unwrap_or_default(),
        }
    }
}

// TODO(fd): Remove after `RestoredVfioConfig` is deprecated and removed.
impl From<RestoredVfioConfig> for ExternalFdsEntry {
    fn from(value: RestoredVfioConfig) -> Self {
        ExternalFdsEntry {
            target: ExternalFdTarget::Vfio { id: value.id },
            expected_fds: 1,
            // `RestoredVfioConfig` may contain valid file descriptors if passed via CLI.
            received_fds: value
                .fd
                .filter(|fd: &RawFd| *fd != -1)
                .map(|fd| vec![fd])
                .unwrap_or_default(),
        }
    }
}

/// File descriptors provided by either the API or CLI.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExternalFds {
    #[serde(default)]
    external_fds: Vec<ExternalFdsEntry>,
}

impl From<Vec<ExternalFdsEntry>> for ExternalFds {
    fn from(value: Vec<ExternalFdsEntry>) -> Self {
        Self {
            external_fds: value,
        }
    }
}

impl ExternalFds {
    /// Takes the entry associated with `target` out of `Self`, if present.
    pub(crate) fn take_entry(&mut self, target: &ExternalFdTarget) -> Option<ExternalFdsEntry> {
        let position = self
            .external_fds
            .iter()
            .position(|entry| &entry.target == target)?;
        Some(self.external_fds.swap_remove(position))
    }

    /// Returns a reference to the entry associated with `target`, if present.
    pub(crate) fn entry(&self, target: &ExternalFdTarget) -> Option<&ExternalFdsEntry> {
        self.external_fds
            .iter()
            .find(|entry| &entry.target == target)
    }

    #[cfg(test)]
    /// Returns all entries as a slice.
    pub(crate) fn as_slice(&self) -> &[ExternalFdsEntry] {
        self.external_fds.as_slice()
    }

    /// Takes all file descriptors out of `Self`.
    ///
    /// Preserves the order to allow ingesting the file descriptors again.
    pub fn take_raw_fds(&mut self) -> Vec<RawFd> {
        self.external_fds
            .iter_mut()
            .flat_map(|fd| mem::take(&mut fd.received_fds))
            .collect()
    }

    /// Returns true if `Self` contains no entries.
    pub(crate) fn is_empty(&self) -> bool {
        self.external_fds.is_empty()
    }

    // TODO(fd): Remove after `RestoredNetConfig` is deprecated and removed.
    /// Imports [`RestoredNetConfig`] into `Self`.
    pub(crate) fn import_restored_net_configs(
        &mut self,
        restored_net_configs: &mut Option<Vec<RestoredNetConfig>>,
    ) {
        if let Some(restored_net_configs) = mem::take(restored_net_configs) {
            self.external_fds
                .splice(0..0, restored_net_configs.into_iter().map(Into::into));
        }
    }

    // TODO(fd): Remove after `RestoredVfioConfig` is deprecated and removed.
    /// Imports [`RestoredVfioConfig`] into `Self`.
    pub(crate) fn import_restored_vfio_configs(
        &mut self,
        restored_vfio_configs: &mut Option<Vec<RestoredVfioConfig>>,
    ) {
        if let Some(restored_vfio_configs) = mem::take(restored_vfio_configs) {
            self.external_fds
                .splice(0..0, restored_vfio_configs.into_iter().map(Into::into));
        }
    }

    // TODO(fd): Remove after `RestoreConfig::iommufd_fd` is deprecated and removed.
    /// Imports [`RestoreConfig::iommufd_fd`] into `Self`.
    pub(crate) fn import_restored_iommufd_fd(&mut self, iommufd_fd: &mut Option<i32>) {
        if let Some(iommufd_fd) = mem::take(iommufd_fd) {
            let entry = ExternalFdsEntry {
                target: ExternalFdTarget::Iommu,
                expected_fds: 1,
                // May contain valid file descriptors if passed via CLI.
                received_fds: if iommufd_fd == -1 {
                    vec![]
                } else {
                    vec![iommufd_fd]
                },
            };
            self.external_fds.insert(0, entry);
        }
    }
}

impl From<TupleList<ExternalFdTarget, Vec<u64>>> for ExternalFds {
    fn from(value: TupleList<ExternalFdTarget, Vec<u64>>) -> Self {
        Self {
            external_fds: value
                .0
                .into_iter()
                .map(|Tuple(target, fds)| {
                    ExternalFdsEntry::new(target, fds.iter().map(|fd| *fd as RawFd).collect())
                })
                .collect(),
        }
    }
}

/// Errors that can occur when updating file descriptors via [`UpdateFds`] or [`UpdateFdsComponent`].
#[derive(Error, Debug, Eq, PartialEq)]
pub enum FdUpdateError {
    /// Mismatch between expected and actual file descriptor number.
    #[error(
        "Mismatch between expected and actual file descriptor number for target \"{target:?}\": actual: {actual}, expected: {expected}"
    )]
    FdAmountMismatch {
        target: ExternalFdTarget,
        expected: usize,
        actual: usize,
    },
    /// Target didn't expect file descriptors.
    #[error("Target didn't expect file descriptors: {0:?}")]
    UnexpectedFds(ExternalFdTarget),
    /// Target without id expected file descriptors
    #[error("Target without id expected file descriptors")]
    MissingId,
    /// Missing file descriptors for target.
    #[error("Missing file descriptors for target: {0:?}")]
    MissingFds(ExternalFdTarget),
    /// Unused file descriptors.
    #[error("File descriptors for the following targets were unused: {0:?}")]
    SuperfluousFds(Vec<ExternalFdTarget>),
    /// Duplicate entry.
    #[error("Duplicate entry for target: {0:?}")]
    DuplicatedTargetEntry(ExternalFdTarget),
    /// VFIO file descriptors require an IOMMU file descriptor.
    #[error("VFIO file descriptors require an IOMMU file descriptor")]
    VfioFdsWithoutIommuFd,
    /// IOMMU file descriptor requires VFIO file descriptors.
    #[error("IOMMU file descriptor requires VFIO file descriptors")]
    IommuFdWithoutVfioFds,
    /// `iommufd_fd` was provided without also enabling the iommufd backend.
    #[error("Platform `iommufd_fd=<fd>` requires `iommufd=on`")]
    IommufdFdRequiresIommufd,
}

/// Errors that can occur when
#[derive(Error, Debug, Eq, PartialEq)]
pub enum IngestScmRightsError {
    /// Less file descriptors provided than expected.
    #[error("Less file descriptors provided than expected")]
    TooLittleFds,
    /// More file descriptors provided than expected.
    #[error("More file descriptors provided than expected")]
    TooManyFds,
}

/// Trait to process file descriptors provided by `SCM_RIGHTS`.
///
/// After deserialization in the API, internal file descriptors are invalid.
/// The trait allows updating those stale file descriptors with valid ones provided by `SCM_RIGHTS`.
pub(crate) trait IngestScmRights {
    /// Consumes `files` and updates all internal file descriptors.
    fn ingest_scm_rights(&mut self, files: Vec<File>) -> Result<(), IngestScmRightsError>;
}

impl IngestScmRights for NetConfig {
    fn ingest_scm_rights(&mut self, files: Vec<File>) -> Result<(), IngestScmRightsError> {
        let fds: Vec<RawFd> = files.into_iter().map(IntoRawFd::into_raw_fd).collect();

        if fds.is_empty() {
            self.fds = None;
        } else {
            self.fds = Some(fds);
        }

        Ok(())
    }
}

impl IngestScmRights for DeviceConfig {
    fn ingest_scm_rights(&mut self, files: Vec<File>) -> Result<(), IngestScmRightsError> {
        if files.len() > 1 {
            Err(IngestScmRightsError::TooManyFds)
        } else {
            self.fd = files.into_iter().map(IntoRawFd::into_raw_fd).next();
            Ok(())
        }
    }
}

impl IngestScmRights for VmReceiveMigrationData {
    fn ingest_scm_rights(&mut self, files: Vec<File>) -> Result<(), IngestScmRightsError> {
        // TODO(fd): Remove after `vfio_fds` is deprecated and removed.
        self.external_fds
            .import_restored_iommufd_fd(&mut self.iommufd_fd);

        // TODO(fd): Remove after `vfio_fds` is deprecated and removed.
        self.external_fds
            .import_restored_vfio_configs(&mut self.vfio_fds);

        self.external_fds.ingest_scm_rights(files)
    }
}

impl IngestScmRights for RestoreConfig {
    fn ingest_scm_rights(&mut self, files: Vec<File>) -> Result<(), IngestScmRightsError> {
        // TODO(fd): Remove after `iommufd_fd` is deprecated and removed.
        self.external_fds
            .import_restored_iommufd_fd(&mut self.iommufd_fd);

        // TODO(fd): Remove after `vfio_fds` is deprecated and removed.
        self.external_fds
            .import_restored_vfio_configs(&mut self.vfio_fds);

        // TODO(fd): Remove after `net_fds` is deprecated and removed.
        self.external_fds
            .import_restored_net_configs(&mut self.net_fds);

        self.external_fds.ingest_scm_rights(files)
    }
}

impl IngestScmRights for VmConfig {
    fn ingest_scm_rights(&mut self, _files: Vec<File>) -> Result<(), IngestScmRightsError> {
        Ok(())
    }
}

impl IngestScmRights for ExternalFds {
    fn ingest_scm_rights(&mut self, mut files: Vec<File>) -> Result<(), IngestScmRightsError> {
        self.external_fds
            .iter_mut()
            .try_for_each(|entry| entry.update_from_scm_rights(&mut files))?;
        if files.is_empty() {
            Ok(())
        } else {
            Err(IngestScmRightsError::TooManyFds)
        }
    }
}

/// Helper trait for [`UpdateFds`], implemented for members of [`VmConfig`].
pub(crate) trait UpdateFdsComponent {
    fn validate_fds(
        &self,
        external_fds: &ExternalFds,
        to_validate: &mut BTreeSet<ExternalFdTarget>,
        operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError>;

    fn update_fds(
        &mut self,
        external_fds: &mut ExternalFds,
        operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError>;
}

pub(crate) trait UpdateFds {
    fn validate_fds(
        &self,
        external_fds: &ExternalFds,
        operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError>;
    fn update_fds(
        &mut self,
        external_fds: ExternalFds,
        operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError>;
}

impl UpdateFdsComponent for NetConfig {
    fn validate_fds(
        &self,
        external_fds: &ExternalFds,
        to_validate: &mut BTreeSet<ExternalFdTarget>,
        _operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        let Some(id) = &self.pci_common.id else {
            return if self.fds.is_some() {
                Err(FdUpdateError::MissingId)
            } else {
                Ok(())
            };
        };

        let target = ExternalFdTarget::Net { id: id.clone() };

        let Some(net_fds) = &self.fds else {
            return if external_fds.entry(&target).is_some() {
                Err(FdUpdateError::UnexpectedFds(target))
            } else {
                Ok(())
            };
        };

        let Some(received_fds) = external_fds.entry(&target) else {
            return Err(FdUpdateError::MissingFds(target));
        };

        if net_fds.len() != received_fds.fds().len() {
            return Err(FdUpdateError::FdAmountMismatch {
                target,
                expected: net_fds.len(),
                actual: received_fds.fds().len(),
            });
        }

        to_validate.remove(&target);

        Ok(())
    }

    fn update_fds(
        &mut self,
        external_fds: &mut ExternalFds,
        _operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        let Some(id) = self.pci_common.id.as_ref() else {
            return Ok(());
        };

        let target = ExternalFdTarget::Net { id: id.clone() };

        let Some(mut received_fds) = external_fds.take_entry(&target) else {
            return Ok(());
        };

        self.fds = Some(received_fds.take_fds());

        Ok(())
    }
}

impl UpdateFdsComponent for DeviceConfig {
    fn validate_fds(
        &self,
        external_fds: &ExternalFds,
        to_validate: &mut BTreeSet<ExternalFdTarget>,
        operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        let Some(id) = &self.pci_common.id else {
            return if self.fd.is_some() {
                Err(FdUpdateError::MissingId)
            } else {
                Ok(())
            };
        };

        let target = ExternalFdTarget::Vfio { id: id.clone() };

        let received_fds = match operation {
            ExternalFdOperation::Restore => {
                match (self.fd.is_some(), external_fds.entry(&target)) {
                    (true | false, Some(received_fds)) => received_fds,
                    (true, None) => return Err(FdUpdateError::MissingFds(target)),
                    (false, None) => return Ok(()),
                }
            }
            ExternalFdOperation::ReceiveMigration => {
                let Some(received_fds) = external_fds.entry(&target) else {
                    return Err(FdUpdateError::MissingFds(target));
                };
                received_fds
            }
            ExternalFdOperation::VmCreate => {
                match (self.fd.is_some(), external_fds.entry(&target)) {
                    (true, Some(received_fds)) => received_fds,
                    (false, Some(_)) => {
                        return Err(FdUpdateError::SuperfluousFds(vec![target]));
                    }
                    (true, None) => return Err(FdUpdateError::MissingFds(target)),
                    (false, None) => return Ok(()),
                }
            }
        };

        if received_fds.fds().len() != 1 {
            return Err(FdUpdateError::FdAmountMismatch {
                target,
                expected: 1,
                actual: received_fds.fds().len(),
            });
        }

        to_validate.remove(&target);

        Ok(())
    }

    fn update_fds(
        &mut self,
        external_fds: &mut ExternalFds,
        _operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        let Some(id) = self.pci_common.id.as_ref() else {
            return Ok(());
        };

        let target = ExternalFdTarget::Vfio { id: id.clone() };

        let Some(mut received_fds) = external_fds.take_entry(&target) else {
            return Ok(());
        };

        self.fd = Some(
            received_fds
                .take_fds()
                .pop()
                .expect("Should be checked during validation"),
        );
        self.path = None;

        Ok(())
    }
}

impl UpdateFdsComponent for PlatformConfig {
    fn validate_fds(
        &self,
        external_fds: &ExternalFds,
        to_validate: &mut BTreeSet<ExternalFdTarget>,
        _operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        let target = ExternalFdTarget::Iommu;
        let Some(received_fds) = external_fds.entry(&target) else {
            return Ok(());
        };

        if !self.iommufd {
            return Err(FdUpdateError::IommufdFdRequiresIommufd);
        }

        match received_fds.fds().len() {
            1 => {
                to_validate.remove(&target);
            }
            len => {
                return Err(FdUpdateError::FdAmountMismatch {
                    target,
                    expected: 1,
                    actual: len,
                });
            }
        }

        Ok(())
    }

    fn update_fds(
        &mut self,
        external_fds: &mut ExternalFds,
        _operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        if let Some(mut received_fds) = external_fds.take_entry(&ExternalFdTarget::Iommu) {
            self.iommufd_fd = Some(
                received_fds
                    .take_fds()
                    .pop()
                    .expect("Should be checked during validation"),
            );
        }

        Ok(())
    }
}

impl UpdateFds for VmConfig {
    fn validate_fds(
        &self,
        external_fds: &ExternalFds,
        operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        let mut to_validate = BTreeSet::new();
        external_fds.external_fds.iter().try_for_each(|entry| {
            if to_validate.insert(entry.target.clone()) {
                Ok(())
            } else {
                Err(FdUpdateError::DuplicatedTargetEntry(entry.target.clone()))
            }
        })?;

        let iommu_fd = to_validate.contains(&ExternalFdTarget::Iommu);
        let vfio_fd = to_validate
            .iter()
            .any(|target| matches!(target, ExternalFdTarget::Vfio { .. }));

        self.net.iter().try_for_each(|net_configs| {
            net_configs.iter().try_for_each(|net_config| {
                net_config.validate_fds(external_fds, &mut to_validate, operation)
            })
        })?;

        self.devices.iter().try_for_each(|device_configs| {
            device_configs.iter().try_for_each(|device_config| {
                device_config.validate_fds(external_fds, &mut to_validate, operation)
            })
        })?;

        self.platform.iter().try_for_each(|platform_config| {
            platform_config.validate_fds(external_fds, &mut to_validate, operation)
        })?;

        if vfio_fd && !iommu_fd {
            return Err(FdUpdateError::VfioFdsWithoutIommuFd);
        }

        if iommu_fd && !vfio_fd {
            return Err(FdUpdateError::IommuFdWithoutVfioFds);
        }

        if to_validate.is_empty() {
            Ok(())
        } else {
            Err(FdUpdateError::SuperfluousFds(
                to_validate.into_iter().collect(),
            ))
        }
    }

    fn update_fds(
        &mut self,
        mut external_fds: ExternalFds,
        operation: ExternalFdOperation,
    ) -> Result<(), FdUpdateError> {
        // Validate before updating to avoid TOCTOU issues.
        self.validate_fds(&external_fds, operation)?;

        self.net.iter_mut().try_for_each(|net_configs| {
            net_configs
                .iter_mut()
                .try_for_each(|net_config| net_config.update_fds(&mut external_fds, operation))
        })?;

        self.devices.iter_mut().try_for_each(|device_configs| {
            device_configs.iter_mut().try_for_each(|device_config| {
                device_config.update_fds(&mut external_fds, operation)
            })
        })?;

        self.platform.iter_mut().try_for_each(|platform_config| {
            platform_config.update_fds(&mut external_fds, operation)
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::os::fd::RawFd;
    use std::str::FromStr;

    use option_parser::{OptionParser, TupleList};
    use serde::{Deserialize, Serialize};

    use crate::external_fds::{
        ExternalFdTarget, ExternalFds, ExternalFdsEntry, ParseExternalFdTargetError,
    };

    pub(crate) fn net_target(id: &str) -> ExternalFdTarget {
        ExternalFdTarget::Net { id: id.to_owned() }
    }

    pub(crate) fn vfio_target(id: &str) -> ExternalFdTarget {
        ExternalFdTarget::Vfio { id: id.to_owned() }
    }

    #[test]
    fn test_parse_external_fd_target() {
        assert_eq!(
            net_target("foo"),
            ExternalFdTarget::from_str("net(foo)").unwrap()
        );

        assert_eq!(
            ParseExternalFdTargetError::EmptyIdent("net()".to_owned()),
            ExternalFdTarget::from_str("net()").unwrap_err()
        );

        assert_eq!(
            ParseExternalFdTargetError::InvalidValue("net((".to_owned()),
            ExternalFdTarget::from_str("net((").unwrap_err()
        );

        assert_eq!(
            ParseExternalFdTargetError::InvalidValue("net".to_owned()),
            ExternalFdTarget::from_str("net").unwrap_err()
        );
    }

    #[test]
    fn parse_external_fds() {
        let mut parser = OptionParser::new();
        parser.add("external_fds");
        parser
            .parse("external_fds=[net(1)@[1,2],net(2)@[3,4]]")
            .unwrap();

        let external_fds: ExternalFds = parser
            .convert::<TupleList<ExternalFdTarget, Vec<u64>>>("external_fds")
            .unwrap()
            .unwrap()
            .into();

        assert_eq!(
            external_fds,
            ExternalFds {
                external_fds: vec![
                    ExternalFdsEntry::new::<RawFd>(net_target("1"), vec![1, 2],),
                    ExternalFdsEntry::new::<RawFd>(net_target("2"), vec![3, 4],),
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
                    ExternalFdsEntry::new::<RawFd>(net_target("1"), vec![1, 2]),
                    ExternalFdsEntry::new::<RawFd>(net_target("2"), vec![3, 4]),
                ],
            },
        })
        .unwrap();

        assert_eq!(
            serialized,
            r#"{"external_fds":[{"target":{"Net":{"id":"1"}},"expected_fds":2},{"target":{"Net":{"id":"2"}},"expected_fds":2}]}"#
        );

        let external_fds: Dummy = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            external_fds.external_fds,
            ExternalFds {
                external_fds: vec![
                    ExternalFdsEntry {
                        target: net_target("1"),
                        expected_fds: 2,
                        received_fds: vec![],
                    },
                    ExternalFdsEntry {
                        target: net_target("2"),
                        expected_fds: 2,
                        received_fds: vec![],
                    },
                ]
            }
        );
    }
}
