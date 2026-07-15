use std::path::PathBuf;

use devices::debug_console;
use option_parser::{OptionParser, OptionParserError};
use serde::{Deserialize, Serialize};

use crate::api::types::PciDeviceCommonConfig;
use crate::config::{Error, ValidationError};
use crate::vm_config;

/// Common configuration for plain console configs.
///
/// Independent of PCI or legacy devices.
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CommonConsoleConfig {
    #[serde(default)]
    pub file: Option<PathBuf>,
    pub mode: ConsoleOutputMode,
    #[serde(default)]
    pub socket: Option<PathBuf>,
}

impl From<CommonConsoleConfig> for vm_config::CommonConsoleConfig {
    fn from(value: CommonConsoleConfig) -> Self {
        Self {
            file: value.file,
            mode: value.mode.into(),
            socket: value.socket,
        }
    }
}

impl From<&vm_config::CommonConsoleConfig> for CommonConsoleConfig {
    fn from(value: &vm_config::CommonConsoleConfig) -> Self {
        Self {
            file: value.file.clone(),
            mode: (&value.mode).into(),
            socket: value.socket.clone(),
        }
    }
}

impl CommonConsoleConfig {
    const VALUELESS_OPTIONS: &[&str] = &["off", "pty", "tty", "null"];
    const VALUE_OPTIONS: &[&str] = &["file", "socket"];

    fn parse(console: &str, map_err: impl Fn(OptionParserError) -> Error) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add_all_valueless(Self::VALUELESS_OPTIONS)
            .add_all(Self::VALUE_OPTIONS);
        parser.parse_subset(console).map_err(map_err)?;

        let mut file: Option<PathBuf> = None;
        let mut socket: Option<PathBuf> = None;
        let mut mode: ConsoleOutputMode = ConsoleOutputMode::Off;

        if parser.is_set("off") {
        } else if parser.is_set("pty") {
            mode = ConsoleOutputMode::Pty;
        } else if parser.is_set("tty") {
            mode = ConsoleOutputMode::Tty;
        } else if parser.is_set("null") {
            mode = ConsoleOutputMode::Null;
        } else if parser.is_set("file") {
            mode = ConsoleOutputMode::File;
            file =
                Some(PathBuf::from(parser.get("file").ok_or(
                    Error::Validation(ValidationError::ConsoleFileMissing),
                )?));
        } else if parser.is_set("socket") {
            mode = ConsoleOutputMode::Socket;
            socket = Some(PathBuf::from(parser.get("socket").ok_or(
                Error::Validation(ValidationError::ConsoleSocketPathMissing),
            )?));
        } else {
            return Err(Error::ParseConsoleInvalidModeGiven);
        }

        Ok(Self { mode, file, socket })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ConsoleOutputMode {
    Off,
    Pty,
    Tty,
    File,
    Socket,
    Null,
}

impl From<ConsoleOutputMode> for vm_config::ConsoleOutputMode {
    fn from(value: ConsoleOutputMode) -> Self {
        match value {
            ConsoleOutputMode::Off => Self::Off,
            ConsoleOutputMode::Pty => Self::Pty,
            ConsoleOutputMode::Tty => Self::Tty,
            ConsoleOutputMode::File => Self::File,
            ConsoleOutputMode::Socket => Self::Socket,
            ConsoleOutputMode::Null => Self::Null,
        }
    }
}

impl From<&vm_config::ConsoleOutputMode> for ConsoleOutputMode {
    fn from(value: &vm_config::ConsoleOutputMode) -> Self {
        match value {
            vm_config::ConsoleOutputMode::Off => Self::Off,
            vm_config::ConsoleOutputMode::Pty => Self::Pty,
            vm_config::ConsoleOutputMode::Tty => Self::Tty,
            vm_config::ConsoleOutputMode::File => Self::File,
            vm_config::ConsoleOutputMode::Socket => Self::Socket,
            vm_config::ConsoleOutputMode::Null => Self::Null,
        }
    }
}

/// Configuration for a legacy serial console device.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SerialConfig {
    #[serde(flatten)]
    pub common: CommonConsoleConfig,
}

impl From<SerialConfig> for vm_config::SerialConfig {
    fn from(value: SerialConfig) -> Self {
        Self {
            common: value.common.into(),
        }
    }
}

impl From<&vm_config::SerialConfig> for SerialConfig {
    fn from(value: &vm_config::SerialConfig) -> Self {
        Self {
            common: (&value.common).into(),
        }
    }
}

impl SerialConfig {
    pub const SYNTAX: &str = "Control serial port: \"off|null|pty|tty|file=<path>|socket=<path>\"";

    pub fn parse(serial: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add_all_valueless(CommonConsoleConfig::VALUELESS_OPTIONS)
            .add_all(CommonConsoleConfig::VALUE_OPTIONS);
        parser.parse(serial).map_err(Error::ParseSerial)?;

        let common = CommonConsoleConfig::parse(serial, Error::ParseSerial)?;
        Ok(Self { common })
    }
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            common: CommonConsoleConfig {
                file: None,
                mode: ConsoleOutputMode::Null,
                socket: None,
            },
        }
    }
}

/// Configuration for a virtio-console device.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ConsoleConfig {
    #[serde(flatten)]
    pub common: CommonConsoleConfig,
    #[serde(default, flatten)]
    pub pci_common: PciDeviceCommonConfig,
}

impl From<ConsoleConfig> for vm_config::ConsoleConfig {
    fn from(value: ConsoleConfig) -> Self {
        Self {
            common: value.common.into(),
            pci_common: value.pci_common.into(),
        }
    }
}

impl From<&vm_config::ConsoleConfig> for ConsoleConfig {
    fn from(value: &vm_config::ConsoleConfig) -> Self {
        Self {
            common: (&value.common).into(),
            pci_common: (&value.pci_common).into(),
        }
    }
}

impl ConsoleConfig {
    pub const SYNTAX: &str = "Control (virtio) console: \"off|null|pty|tty|file=<path>,iommu=on|off,id=<device_id>,pci_segment=<segment_id>,pci_device_id=<pci_slot>\"";

    pub fn parse(console: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add_all_valueless(CommonConsoleConfig::VALUELESS_OPTIONS)
            .add_all(CommonConsoleConfig::VALUE_OPTIONS)
            .add_all(PciDeviceCommonConfig::OPTIONS_IOMMU);
        parser.parse(console).map_err(Error::ParseConsole)?;

        let common = CommonConsoleConfig::parse(console, Error::ParseConsole)?;
        let pci_common = PciDeviceCommonConfig::parse(console)?;

        Ok(Self { common, pci_common })
    }
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            common: CommonConsoleConfig {
                file: None,
                mode: ConsoleOutputMode::Tty,
                socket: None,
            },
            pci_common: PciDeviceCommonConfig::default(),
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct DebugConsoleConfig {
    #[serde(default)]
    pub file: Option<PathBuf>,
    pub mode: ConsoleOutputMode,
    /// Optionally dedicated I/O-port, if the default port should not be used.
    pub iobase: Option<u16>,
}

#[cfg(target_arch = "x86_64")]
impl From<DebugConsoleConfig> for vm_config::DebugConsoleConfig {
    fn from(value: DebugConsoleConfig) -> Self {
        Self {
            file: value.file,
            mode: value.mode.into(),
            iobase: value.iobase,
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl From<&vm_config::DebugConsoleConfig> for DebugConsoleConfig {
    fn from(value: &vm_config::DebugConsoleConfig) -> Self {
        Self {
            file: value.file.clone(),
            mode: (&value.mode).into(),
            iobase: value.iobase,
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl DebugConsoleConfig {
    pub fn parse(debug_console_ops: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add_valueless("off")
            .add_valueless("pty")
            .add_valueless("tty")
            .add_valueless("null")
            .add("file")
            .add("iobase");
        parser
            .parse(debug_console_ops)
            .map_err(Error::ParseConsole)?;

        let mut file: Option<PathBuf> = None;
        let mut iobase: Option<u16> = None;
        let mut mode: ConsoleOutputMode = ConsoleOutputMode::Off;

        if parser.is_set("off") {
        } else if parser.is_set("pty") {
            mode = ConsoleOutputMode::Pty;
        } else if parser.is_set("tty") {
            mode = ConsoleOutputMode::Tty;
        } else if parser.is_set("null") {
            mode = ConsoleOutputMode::Null;
        } else if parser.is_set("file") {
            mode = ConsoleOutputMode::File;
            file =
                Some(PathBuf::from(parser.get("file").ok_or(
                    Error::Validation(ValidationError::ConsoleFileMissing),
                )?));
        } else {
            return Err(Error::ParseConsoleInvalidModeGiven);
        }

        if parser.is_set("iobase")
            && let Some(iobase_opt) = parser.get("iobase")
        {
            if !iobase_opt.starts_with("0x") {
                return Err(Error::Validation(ValidationError::InvalidIoPortHex(
                    iobase_opt,
                )));
            }
            iobase =
                Some(u16::from_str_radix(&iobase_opt[2..], 16).map_err(|_| {
                    Error::Validation(ValidationError::InvalidIoPortHex(iobase_opt))
                })?);
        }

        Ok(Self { file, mode, iobase })
    }
}

#[cfg(target_arch = "x86_64")]
impl Default for DebugConsoleConfig {
    fn default() -> Self {
        Self {
            file: None,
            mode: ConsoleOutputMode::Off,
            iobase: Some(debug_console::DEFAULT_PORT as u16),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_parsing() -> Result<(), Error> {
        let console_config = |mode, file, socket, iommu| ConsoleConfig {
            common: CommonConsoleConfig { file, mode, socket },
            pci_common: PciDeviceCommonConfig {
                iommu,
                ..Default::default()
            },
        };

        ConsoleConfig::parse("").unwrap_err();
        ConsoleConfig::parse("badmode").unwrap_err();
        assert_eq!(
            ConsoleConfig::parse("off")?,
            console_config(ConsoleOutputMode::Off, None, None, false)
        );
        assert_eq!(
            ConsoleConfig::parse("pty")?,
            console_config(ConsoleOutputMode::Pty, None, None, false)
        );
        assert_eq!(
            ConsoleConfig::parse("tty")?,
            console_config(ConsoleOutputMode::Tty, None, None, false)
        );
        assert_eq!(
            ConsoleConfig::parse("null")?,
            console_config(ConsoleOutputMode::Null, None, None, false)
        );
        assert_eq!(
            ConsoleConfig::parse("file=/tmp/console")?,
            console_config(
                ConsoleOutputMode::File,
                Some(PathBuf::from("/tmp/console")),
                None,
                false
            )
        );
        assert_eq!(
            ConsoleConfig::parse("null,iommu=on")?,
            console_config(ConsoleOutputMode::Null, None, None, true)
        );
        assert_eq!(
            ConsoleConfig::parse("file=/tmp/console,iommu=on")?,
            console_config(
                ConsoleOutputMode::File,
                Some(PathBuf::from("/tmp/console")),
                None,
                true
            )
        );
        assert_eq!(
            ConsoleConfig::parse("socket=/tmp/serial.sock,iommu=on")?,
            console_config(
                ConsoleOutputMode::Socket,
                None,
                Some(PathBuf::from("/tmp/serial.sock")),
                true
            )
        );
        Ok(())
    }
}
