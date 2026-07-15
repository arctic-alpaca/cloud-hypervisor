use option_parser::{ByteSized, OptionParser, Toggle};
use serde::{Deserialize, Serialize};

use crate::api::types::PciDeviceCommonConfig;
use crate::config::Error;
use crate::vm_config;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct BalloonConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub size: u64,
    /// Option to deflate the balloon in case the guest is out of memory.
    #[serde(default)]
    pub deflate_on_oom: bool,
    /// Option to enable free page reporting from the guest.
    #[serde(default)]
    pub free_page_reporting: bool,
}

impl From<BalloonConfig> for vm_config::BalloonConfig {
    fn from(value: BalloonConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            size: value.size,
            deflate_on_oom: value.deflate_on_oom,
            free_page_reporting: value.free_page_reporting,
        }
    }
}

impl From<&vm_config::BalloonConfig> for BalloonConfig {
    fn from(value: &vm_config::BalloonConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            size: value.size,
            deflate_on_oom: value.deflate_on_oom,
            free_page_reporting: value.free_page_reporting,
        }
    }
}

impl BalloonConfig {
    pub const SYNTAX: &'static str = "Balloon parameters \"size=<balloon_size>,deflate_on_oom=on|off,\
        free_page_reporting=on|off,iommu=on|off,id=<device_id>,pci_segment=<segment_id>,\
        pci_device_id=<pci_slot>\"";

    pub fn parse(balloon: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add("size");
        parser.add("deflate_on_oom");
        parser.add("free_page_reporting");
        parser.add_all(PciDeviceCommonConfig::OPTIONS_IOMMU);
        parser.parse(balloon).map_err(Error::ParseBalloon)?;

        let size = parser
            .convert::<ByteSized>("size")
            .map_err(Error::ParseBalloon)?
            .map_or(0, |v| v.0);

        let deflate_on_oom = parser
            .convert::<Toggle>("deflate_on_oom")
            .map_err(Error::ParseBalloon)?
            .unwrap_or(Toggle(false))
            .0;

        let free_page_reporting = parser
            .convert::<Toggle>("free_page_reporting")
            .map_err(Error::ParseBalloon)?
            .unwrap_or(Toggle(false))
            .0;

        let pci_common = PciDeviceCommonConfig::parse(balloon)?;

        Ok(BalloonConfig {
            pci_common,
            size,
            deflate_on_oom,
            free_page_reporting,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_balloon() -> Result<(), Error> {
        assert_eq!(
            BalloonConfig::parse(
                "size=128M,deflate_on_oom=on,free_page_reporting=on,pci_segment=1,pci_device_id=7"
            )?,
            BalloonConfig {
                pci_common: PciDeviceCommonConfig {
                    pci_segment: 1,
                    pci_device_id: Some(7),
                    ..Default::default()
                },
                size: 128 << 20,
                deflate_on_oom: true,
                free_page_reporting: true,
            }
        );

        assert_eq!(
            BalloonConfig::parse("size=0,iommu=on")?,
            BalloonConfig {
                pci_common: PciDeviceCommonConfig {
                    iommu: true,
                    ..Default::default()
                },
                size: 0,
                deflate_on_oom: false,
                free_page_reporting: false,
            }
        );

        Ok(())
    }
}
