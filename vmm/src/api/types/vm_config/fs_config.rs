use std::path::PathBuf;

use option_parser::OptionParser;
use serde::{Deserialize, Serialize};
use virtio_devices::vhost_user;

use crate::api::types::PciDeviceCommonConfig;
use crate::config::Error;
use crate::vm_config;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct FsConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub tag: String,
    pub socket: PathBuf,
    #[serde(default = "default_fsconfig_num_queues")]
    pub num_queues: usize,
    #[serde(default = "default_fsconfig_queue_size")]
    pub queue_size: u16,
}

impl From<FsConfig> for vm_config::FsConfig {
    fn from(value: FsConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            tag: value.tag,
            socket: value.socket,
            num_queues: value.num_queues,
            queue_size: value.queue_size,
        }
    }
}

impl From<&vm_config::FsConfig> for FsConfig {
    fn from(value: &vm_config::FsConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            tag: value.tag.clone(),
            socket: value.socket.clone(),
            num_queues: value.num_queues,
            queue_size: value.queue_size,
        }
    }
}

impl FsConfig {
    pub const SYNTAX: &'static str = "virtio-fs parameters \
    \"tag=<tag_name>,socket=<socket_path>,num_queues=<number_of_queues>,\
    queue_size=<size_of_each_queue>,id=<device_id>,\
    pci_segment=<segment_id>,pci_device_id=<pci_slot>\"";

    pub fn parse(fs: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("tag")
            .add("queue_size")
            .add("num_queues")
            .add("socket")
            .add_all(PciDeviceCommonConfig::OPTIONS);
        parser.parse(fs).map_err(Error::ParseFileSystem)?;

        let tag = parser.get("tag").ok_or(Error::ParseFsTagMissing)?;
        if tag.len() > vhost_user::VIRTIO_FS_TAG_LEN {
            return Err(Error::ParseFsTagTooLong);
        }
        let socket = PathBuf::from(parser.get("socket").ok_or(Error::ParseFsSockMissing)?);

        let queue_size = parser
            .convert("queue_size")
            .map_err(Error::ParseFileSystem)?
            .unwrap_or_else(default_fsconfig_queue_size);
        let num_queues = parser
            .convert("num_queues")
            .map_err(Error::ParseFileSystem)?
            .unwrap_or_else(default_fsconfig_num_queues);

        let pci_common = PciDeviceCommonConfig::parse(fs)?;

        Ok(FsConfig {
            pci_common,
            tag,
            socket,
            num_queues,
            queue_size,
        })
    }
}

pub fn default_fsconfig_num_queues() -> usize {
    1
}

pub fn default_fsconfig_queue_size() -> u16 {
    1024
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs_fixture() -> FsConfig {
        FsConfig {
            pci_common: PciDeviceCommonConfig::default(),
            socket: PathBuf::from("/tmp/sock"),
            tag: "mytag".to_owned(),
            num_queues: 1,
            queue_size: 1024,
        }
    }

    #[test]
    fn test_parse_fs() -> Result<(), Error> {
        // "tag" and "socket" must be supplied
        FsConfig::parse("").unwrap_err();
        FsConfig::parse("tag=mytag").unwrap_err();
        FsConfig::parse("socket=/tmp/sock").unwrap_err();
        assert_eq!(FsConfig::parse("tag=mytag,socket=/tmp/sock")?, fs_fixture());
        assert_eq!(
            FsConfig::parse("tag=mytag,socket=/tmp/sock,num_queues=4,queue_size=1024")?,
            FsConfig {
                num_queues: 4,
                queue_size: 1024,
                ..fs_fixture()
            }
        );

        Ok(())
    }
}
