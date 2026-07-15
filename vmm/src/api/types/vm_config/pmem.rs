use std::path::PathBuf;

use option_parser::{ByteSized, OptionParser, Toggle};
use serde::{Deserialize, Serialize};

use crate::api::types::PciDeviceCommonConfig;
use crate::config::Error;
use crate::vm_config;

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PmemConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub file: PathBuf,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub discard_writes: bool,
}

impl From<PmemConfig> for vm_config::PmemConfig {
    fn from(value: PmemConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            file: value.file,
            size: value.size,
            discard_writes: value.discard_writes,
        }
    }
}

impl From<&vm_config::PmemConfig> for PmemConfig {
    fn from(value: &vm_config::PmemConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            file: value.file.clone(),
            size: value.size,
            discard_writes: value.discard_writes,
        }
    }
}

impl PmemConfig {
    pub const SYNTAX: &'static str = "Persistent memory parameters \
    \"file=<backing_file_path>,size=<persistent_memory_size>,iommu=on|off,\
    discard_writes=on|off,id=<device_id>,\
    pci_segment=<segment_id>,pci_device_id=<pci_slot>\"";

    pub fn parse(pmem: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("size")
            .add("file")
            .add("discard_writes")
            .add_all(PciDeviceCommonConfig::OPTIONS_IOMMU);
        parser.parse(pmem).map_err(Error::ParsePersistentMemory)?;

        Ok(Self {
            pci_common: PciDeviceCommonConfig::parse(pmem)?,
            file: PathBuf::from(parser.get("file").ok_or(Error::ParsePmemFileMissing)?),
            size: parser
                .convert::<ByteSized>("size")
                .map_err(Error::ParsePersistentMemory)?
                .map(|value| value.0),
            discard_writes: parser
                .convert::<Toggle>("discard_writes")
                .map_err(Error::ParsePersistentMemory)?
                .unwrap_or(Toggle(false))
                .0,
        })
    }
}
