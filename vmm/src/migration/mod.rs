// Copyright © 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::fs::File;
use std::io::Read;
use std::num::{NonZeroU32, NonZeroU64};
use std::path::PathBuf;
use std::result;
use std::time::Duration;

use anyhow::{Context, anyhow};
use thiserror::Error;
use vm_migration::tls::{TlsEndpoint, validate_tls_dir};
use vm_migration::{MigratableError, Snapshot};

use crate::api;
#[cfg(all(target_arch = "x86_64", feature = "guest_debug"))]
use crate::coredump::GuestDebuggableError;
use crate::migration::transport::{
    MAX_MIGRATION_CONNECTIONS, TcpAddressParseError, tcp_address_to_server_name,
};
use crate::vm::VmSnapshot;
use crate::vm_config::VmConfig;

pub(crate) mod transport;
pub(crate) mod worker;

pub const SNAPSHOT_STATE_FILE: &str = "state.json";
pub const SNAPSHOT_CONFIG_FILE: &str = "config.json";

pub fn url_to_path(url: &str) -> result::Result<PathBuf, MigratableError> {
    let path: PathBuf = url
        .strip_prefix("file://")
        .ok_or_else(|| {
            MigratableError::MigrateSend(anyhow!("Could not extract path from URL: {url}"))
        })
        .map(|s| s.into())?;

    if !path.is_dir() {
        return Err(MigratableError::MigrateSend(anyhow!(
            "Destination is not a directory: {path:?}"
        )));
    }

    Ok(path)
}

#[cfg(all(target_arch = "x86_64", feature = "guest_debug"))]
pub fn url_to_file(url: &str) -> result::Result<PathBuf, GuestDebuggableError> {
    let file: PathBuf = url
        .strip_prefix("file://")
        .ok_or_else(|| {
            GuestDebuggableError::Coredump(anyhow!("Could not extract file from URL: {url}"))
        })
        .map(|s| s.into())?;

    Ok(file)
}

pub fn recv_vm_config(source_url: &str) -> result::Result<VmConfig, MigratableError> {
    let mut vm_config_path = url_to_path(source_url)?;

    vm_config_path.push(SNAPSHOT_CONFIG_FILE);

    // Try opening the snapshot file
    let mut vm_config_file = File::open(&vm_config_path)
        .with_context(|| format!("Error opening VM config snapshot file {vm_config_path:?}"))
        .map_err(MigratableError::MigrateReceive)?;
    let mut bytes = Vec::new();
    vm_config_file
        .read_to_end(&mut bytes)
        .with_context(|| format!("Error reading VM config snapshot file {vm_config_path:?}"))
        .map_err(MigratableError::MigrateReceive)?;

    serde_json::from_slice(&bytes)
        .context("Error deserialising VM config snapshot")
        .map_err(MigratableError::MigrateReceive)
}

pub fn recv_vm_state(source_url: &str) -> result::Result<Snapshot, MigratableError> {
    let mut vm_state_path = url_to_path(source_url)?;

    vm_state_path.push(SNAPSHOT_STATE_FILE);

    // Try opening the snapshot file
    let mut vm_state_file = File::open(&vm_state_path)
        .with_context(|| format!("Error opening VM state snapshot file {vm_state_path:?}"))
        .map_err(MigratableError::MigrateReceive)?;
    let mut bytes = Vec::new();
    vm_state_file
        .read_to_end(&mut bytes)
        .with_context(|| format!("Error reading VM state snapshot file {vm_state_path:?}"))
        .map_err(MigratableError::MigrateReceive)?;

    serde_json::from_slice(&bytes)
        .context("Error deserialising VM state snapshot")
        .map_err(MigratableError::MigrateReceive)
}

pub fn get_vm_snapshot(snapshot: &Snapshot) -> result::Result<VmSnapshot, MigratableError> {
    if let Some(snapshot_data) = snapshot.snapshot_data.as_ref() {
        return snapshot_data.to_state();
    }

    Err(MigratableError::Restore(anyhow!(
        "Could not find VM config snapshot section"
    )))
}

#[derive(Debug, Error)]
pub enum VmSendMigrationConfigError {
    #[error(
        "Error validating send migration parameters: destination_url must use tcp:<host>:<port> or unix:<path>."
    )]
    InvalidDestinationUrl(#[source] TcpAddressParseError),

    #[error("Error validating send migration parameters: {0}")]
    ValidationError(String),
}

/// Configuration for an outgoing migration.
#[derive(Clone, Debug)]
pub struct VmSendMigrationData {
    /// Migration destination, e.g. `tcp:<host>:<port>` or `unix:/path/to/socket`.
    pub destination_url: String,
    /// Send memory across socket without copying
    pub local: bool,
    /// The maximum downtime the migration aims for.
    ///
    /// Usually, on the order of a few hundred milliseconds.
    downtime_ms: NonZeroU64,
    /// The timeout for the migration, i.e., the maximum duration.
    timeout_s: NonZeroU64,
    /// The timeout strategy for the migration.
    pub timeout_strategy: TimeoutStrategy,
    /// The number of parallel TCP connections for migration.
    ///
    /// Must be between 1 and `MAX_MIGRATION_CONNECTIONS` inclusive.
    pub connections: NonZeroU32,
    /// Directory containing the TLS client certificate (`client-cert.pem`),
    /// the TLS client key (`client-key.pem`), and the client's TLS root CA
    /// certificate (`ca-cert.pem`).
    ///
    /// If this is `Some`, the migration is instructed to use mTLS.
    pub tls_dir: Option<PathBuf>,
    /// Memory transfer mode.
    pub memory_mode: MigrationMode,
}

impl TryFrom<api::types::VmSendMigrationData> for VmSendMigrationData {
    type Error = VmSendMigrationConfigError;

    fn try_from(value: api::types::VmSendMigrationData) -> Result<Self, Self::Error> {
        let config = Self {
            destination_url: value.destination_url,
            local: value.local,
            downtime_ms: value.downtime_ms,
            timeout_s: value.timeout_s,
            timeout_strategy: value.timeout_strategy.into(),
            connections: value.connections,
            tls_dir: value.tls_dir,
            memory_mode: value.memory_mode.into(),
        };
        config.validate()?;
        Ok(config)
    }
}

impl VmSendMigrationData {
    pub fn downtime(&self) -> Duration {
        Duration::from_millis(self.downtime_ms.get())
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_s.get())
    }

    pub fn validate(&self) -> Result<(), VmSendMigrationConfigError> {
        if let Some(addr) = self.destination_url.strip_prefix("tcp:") {
            tcp_address_to_server_name(addr)
                .map_err(VmSendMigrationConfigError::InvalidDestinationUrl)?;
        } else if self
            .destination_url
            .strip_prefix("unix:")
            .is_some_and(|path| !path.is_empty())
        {
            if self.connections.get() > 1 {
                return Err(VmSendMigrationConfigError::ValidationError(
                    "UNIX sockets and connections option cannot be used at the same time."
                        .to_string(),
                ));
            }
            if self.tls_dir.is_some() {
                return Err(VmSendMigrationConfigError::ValidationError(
                    "UNIX sockets and TLS encryption cannot be used at the same time.".to_string(),
                ));
            }
        } else {
            return Err(VmSendMigrationConfigError::ValidationError(
                "destination_url must use tcp:<host>:<port> or unix:<path>.".to_string(),
            ));
        }

        if self.connections.get() > MAX_MIGRATION_CONNECTIONS {
            return Err(VmSendMigrationConfigError::ValidationError(format!(
                "connections must not exceed {MAX_MIGRATION_CONNECTIONS}."
            )));
        }

        if self.local {
            if !self.destination_url.starts_with("unix:") {
                return Err(VmSendMigrationConfigError::ValidationError(
                    "local option is only supported with UNIX sockets.".to_string(),
                ));
            }

            if self.connections.get() > 1 {
                return Err(VmSendMigrationConfigError::ValidationError(
                    "local option and connections option cannot be used at the same time."
                        .to_string(),
                ));
            }
        }

        if let Some(tls_dir) = &self.tls_dir {
            validate_tls_dir(tls_dir, TlsEndpoint::Client).map_err(|e| {
                VmSendMigrationConfigError::ValidationError(format!(
                    "invalid TLS configuration for send-migration: {e}"
                ))
            })?;
        }

        if matches!(self.memory_mode, MigrationMode::Postcopy) {
            if self.local {
                return Err(VmSendMigrationConfigError::ValidationError(
                    "memory_mode=postcopy and local options are mutually exclusive.".to_string(),
                ));
            }

            if self.connections.get() > 1 {
                return Err(VmSendMigrationConfigError::ValidationError(
                    "memory_mode=postcopy currently requires a single connection (connections=1)."
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Default, Debug)]
pub struct VmReceiveMigrationData {
    /// URL for the reception of migration state
    pub receiver_url: String,
    /// Directory containing the TLS server certificate (`server-cert.pem`),
    /// the TLS server key (`server-key.pem`), and the server's TLS root CA
    /// certificate (`ca-cert.pem`).
    ///
    /// If this is `Some`, the migration is instructed to use mTLS.
    pub tls_dir: Option<PathBuf>,
    /// Memory transfer mode.
    pub memory_mode: MigrationMode,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum VmReceiveMigrationDataValidationError {
    #[error("Error validating receive migration parameters: {0}")]
    ValidationError(String),
}

impl VmReceiveMigrationData {
    pub fn validate(&self) -> Result<(), VmReceiveMigrationDataValidationError> {
        if let Some(addr) = self.receiver_url.strip_prefix("tcp:") {
            tcp_address_to_server_name(addr).map_err(|e| {
                VmReceiveMigrationDataValidationError::ValidationError(format!(
                    "receiver_url must use tcp:<host>:<port> or unix:<path>: {e}."
                ))
            })?;
        } else if self
            .receiver_url
            .strip_prefix("unix:")
            .is_some_and(|path| !path.is_empty())
        {
            if self.tls_dir.is_some() {
                return Err(VmReceiveMigrationDataValidationError::ValidationError(
                    "UNIX sockets and TLS encryption cannot be used at the same time.".to_string(),
                ));
            }
        } else {
            return Err(VmReceiveMigrationDataValidationError::ValidationError(
                "receiver_url must use tcp:<host>:<port> or unix:<path>.".to_string(),
            ));
        }

        if let Some(tls_dir) = &self.tls_dir {
            validate_tls_dir(tls_dir, TlsEndpoint::Server).map_err(|e| {
                VmReceiveMigrationDataValidationError::ValidationError(format!(
                    "invalid TLS configuration for receive-migration: {e}"
                ))
            })?;
        }

        Ok(())
    }
}

impl TryFrom<api::types::VmReceiveMigrationData> for VmReceiveMigrationData {
    type Error = VmReceiveMigrationDataValidationError;

    fn try_from(value: api::types::VmReceiveMigrationData) -> Result<Self, Self::Error> {
        let config = Self {
            receiver_url: value.receiver_url,
            tls_dir: value.tls_dir,
            memory_mode: value.memory_mode.into(),
        };
        config.validate()?;
        Ok(config)
    }
}

/// Memory transfer mode for a migration.
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub enum MigrationMode {
    /// Transfer all guest memory before the destination resumes.
    #[default]
    Precopy,
    /// Resume the destination first and fault guest pages in on demand.
    /// This is an experimental mode. It uses a single connection even
    /// when parallel connections are configured. Pages are served on
    /// demand, but a background faulting mechanism also pulls in the
    /// remaining pages to speed up completion.
    Postcopy,
}

impl From<api::types::MigrationMode> for MigrationMode {
    fn from(value: api::types::MigrationMode) -> Self {
        match value {
            api::types::MigrationMode::Precopy => Self::Precopy,
            api::types::MigrationMode::Postcopy => Self::Postcopy,
        }
    }
}

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
/// The migration timeout strategy.
///
/// This strategy describes the behavior of the migration when the target
/// downtime can't be reached in the given timeout.
pub enum TimeoutStrategy {
    #[default]
    /// Cancel the migration and keep the VM running on the source.
    Cancel,
    /// Ignore the timeout and migrate anyway.
    Ignore,
}

impl From<api::types::TimeoutStrategy> for TimeoutStrategy {
    fn from(value: api::types::TimeoutStrategy) -> Self {
        match value {
            api::types::TimeoutStrategy::Cancel => Self::Cancel,
            api::types::TimeoutStrategy::Ignore => Self::Ignore,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};
    use std::path::PathBuf;

    use crate::api;
    use crate::migration::transport::TcpAddressParseError;
    use crate::migration::{
        MigrationMode, VmReceiveMigrationData, VmReceiveMigrationDataValidationError,
        VmSendMigrationConfigError, VmSendMigrationData,
    };

    #[test]
    fn test_vm_send_migration_data_validate() {
        // Invalid destination URL scheme is rejected
        assert!(matches!(
            VmSendMigrationData::try_from(
                api::types::VmSendMigrationData::parse("destination_url=tcp:192.168.1.1").unwrap()
            )
            .unwrap_err(),
            VmSendMigrationConfigError::InvalidDestinationUrl(TcpAddressParseError::MissingPort)
        ));
        assert!(matches!(
            VmSendMigrationData::try_from(
                api::types::VmSendMigrationData::parse("destination_url=tcp:[2001:db8::1]")
                    .unwrap()
            )
            .unwrap_err(),
            VmSendMigrationConfigError::InvalidDestinationUrl(
                TcpAddressParseError::MissingPortSeparatorAfterBracketedHost
            )
        ));

        // Excessive numbers of parallel connections are rejected
        let _data = VmSendMigrationData {
            destination_url: "tcp:192.168.1.1:8080".to_string(),
            local: false,
            downtime_ms: NonZeroU64::new(10).unwrap(),
            timeout_s: NonZeroU64::new(10).unwrap(),
            timeout_strategy: Default::default(),
            connections: NonZeroU32::new(129).unwrap(),
            tls_dir: None,
            memory_mode: Default::default(),
        }
        .validate()
        .expect_err("too many connections should be rejected");
        // memory_mode=postcopy + local must be rejected.
        VmSendMigrationData {
            destination_url: "unix:/tmp/sock".to_string(),
            local: true,
            downtime_ms: NonZeroU64::new(10).unwrap(),
            timeout_s: NonZeroU64::new(10).unwrap(),
            timeout_strategy: Default::default(),
            connections: NonZeroU32::new(1).unwrap(),
            tls_dir: None,
            memory_mode: MigrationMode::Postcopy,
        }
        .validate()
        .unwrap_err();
        // memory_mode=postcopy + multi-connection must be rejected.
        VmSendMigrationData {
            destination_url: "tcp:192.168.1.1:8080".to_string(),
            local: false,
            downtime_ms: NonZeroU64::new(10).unwrap(),
            timeout_s: NonZeroU64::new(10).unwrap(),
            timeout_strategy: Default::default(),
            connections: NonZeroU32::new(4).unwrap(),
            tls_dir: None,
            memory_mode: MigrationMode::Postcopy,
        }
        .validate()
        .unwrap_err();
    }

    #[test]
    fn test_vm_receive_migration_data_validate() {
        let tls_dir = tempfile::tempdir().unwrap();
        VmReceiveMigrationData {
            receiver_url: "tcp:192.168.1.1:8080".to_string(),
            tls_dir: Some(tls_dir.path().to_owned()),
            memory_mode: Default::default(),
        }
        .validate()
        .unwrap_err();

        assert_eq!(
            VmReceiveMigrationData::try_from(api::types::VmReceiveMigrationData {
                receiver_url: "file:///tmp/migration".to_owned(),
                ..Default::default()
            })
            .unwrap_err(),
            VmReceiveMigrationDataValidationError::ValidationError(
                "receiver_url must use tcp:<host>:<port> or unix:<path>.".to_owned()
            )
        );

        assert_eq!(
            VmReceiveMigrationData {
                receiver_url: "tcp:192.168.1.1".to_owned(),
                ..Default::default()
            }
            .validate(),
            Err(VmReceiveMigrationDataValidationError::ValidationError(
                "receiver_url must use tcp:<host>:<port> or unix:<path>: Missing TCP port."
                    .to_owned()
            ))
        );

        assert_eq!(
            VmReceiveMigrationData {
                receiver_url: "tcp:[2001:db8::1]".to_owned(),
                ..Default::default()
            }
            .validate(),
            Err(VmReceiveMigrationDataValidationError::ValidationError(
                "receiver_url must use tcp:<host>:<port> or unix:<path>: Missing port separator after bracketed host."
                    .to_owned()
            ))
        );

        assert_eq!(
            VmReceiveMigrationData {
                receiver_url: "unix:/tmp/sock".to_owned(),
                tls_dir: Some(PathBuf::from("/tmp")),
                ..Default::default()
            }
            .validate(),
            Err(VmReceiveMigrationDataValidationError::ValidationError(
                "UNIX sockets and TLS encryption cannot be used at the same time.".to_owned()
            ))
        );
    }
}
