use option_parser::{OptionParser, Toggle};
use serde::{Deserialize, Serialize};

use crate::config::Error;
use crate::vm_config;

mod balloon;
mod console;
mod cpus;
mod devices;
mod disk;
mod fs_config;
mod generic_vhost_user;
mod memory;
mod net;
mod numa;
mod payload;
mod platform;
mod pmem;
mod rate_limiter_group;
mod rng;

pub use balloon::BalloonConfig;
pub use console::{ConsoleConfig, DebugConsoleConfig, SerialConfig};
pub use cpus::CpusConfig;
pub use disk::DiskConfig;
pub use devices::{
    DeviceConfig, IvshmemConfig, LandlockConfig, PciSegmentConfig, PvmemcontrolConfig, RtcConfig,
    TpmConfig, UserDeviceConfig, VdpaConfig, VsockConfig,
};
pub use fs_config::FsConfig;
pub use generic_vhost_user::GenericVhostUserConfig;
pub use memory::MemoryConfig;
pub use net::NetConfig;
pub use numa::NumaConfig;
pub use payload::{FwCfgConfig, PayloadConfig};
pub use platform::PlatformConfig;
pub use pmem::PmemConfig;
pub use rate_limiter_group::RateLimiterGroupConfig;
pub use rng::RngConfig;

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct PciDeviceCommonConfig {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "<&bool as std::ops::Not>::not")]
    pub iommu: bool,
    #[serde(default)]
    pub pci_segment: u16,
    #[serde(default)]
    pub pci_device_id: Option<u8>,
}

impl From<PciDeviceCommonConfig> for vm_config::PciDeviceCommonConfig {
    fn from(value: PciDeviceCommonConfig) -> Self {
        Self {
            id: value.id,
            iommu: value.iommu,
            pci_segment: value.pci_segment,
            pci_device_id: value.pci_device_id,
        }
    }
}

impl From<&vm_config::PciDeviceCommonConfig> for PciDeviceCommonConfig {
    fn from(value: &vm_config::PciDeviceCommonConfig) -> Self {
        Self {
            id: value.id.clone(),
            iommu: value.iommu,
            pci_segment: value.pci_segment,
            pci_device_id: value.pci_device_id,
        }
    }
}

impl PciDeviceCommonConfig {
    const OPTIONS: &[&str] = &["id", "pci_segment", "pci_device_id"];
    pub(crate) const OPTIONS_IOMMU: &[&str] = &["id", "iommu", "pci_segment", "pci_device_id"];

    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add_all(Self::OPTIONS_IOMMU);
        parser
            .parse_subset(input)
            .map_err(Error::ParsePciDeviceCommonConfig)?;

        Ok(Self {
            id: parser.get("id"),
            iommu: parser
                .convert::<Toggle>("iommu")
                .map_err(Error::ParsePciDeviceCommonConfig)?
                .unwrap_or(Toggle(false))
                .0,
            pci_segment: parser
                .convert("pci_segment")
                .map_err(Error::ParsePciDeviceCommonConfig)?
                .unwrap_or_default(),
            pci_device_id: parser
                .convert::<u8>("pci_device_id")
                .map_err(Error::ParsePciDeviceCommonConfig)?,
        })
    }
}
