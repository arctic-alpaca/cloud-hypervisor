use std::path::PathBuf;

use option_parser::OptionParser;
use serde::{Deserialize, Serialize};

use crate::api::types::PciDeviceCommonConfig;
use crate::config::Error;
use crate::vm_config;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RngConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub src: PathBuf,
}

impl From<RngConfig> for vm_config::RngConfig {
    fn from(value: RngConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            src: value.src,
        }
    }
}

impl From<&vm_config::RngConfig> for RngConfig {
    fn from(value: &vm_config::RngConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            src: value.src.clone(),
        }
    }
}

impl RngConfig {
    pub const SYNTAX: &'static str = "Random number generator parameters \"\
        src=<entropy_source_path>,iommu=on|off,pci_segment=<segment_id>,\
        pci_device_id=<pci_slot>\"";

    pub fn parse(rng: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("src")
            .add_all(PciDeviceCommonConfig::OPTIONS_IOMMU);
        parser.parse(rng).map_err(Error::ParseRng)?;

        let src = PathBuf::from(
            parser
                .get("src")
                .unwrap_or_else(|| vm_config::RngConfig::DEFAULT_RNG_SOURCE.to_owned()),
        );

        let pci_common = PciDeviceCommonConfig::parse(rng)?;

        Ok(RngConfig { src, pci_common })
    }
}

impl Default for RngConfig {
    fn default() -> Self {
        RngConfig {
            src: PathBuf::from(vm_config::RngConfig::DEFAULT_RNG_SOURCE),
            pci_common: PciDeviceCommonConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rng() -> Result<(), Error> {
        assert_eq!(RngConfig::parse("")?, RngConfig::default());
        assert_eq!(
            RngConfig::parse("src=/dev/random")?,
            RngConfig {
                src: PathBuf::from("/dev/random"),
                ..Default::default()
            }
        );
        assert_eq!(
            RngConfig::parse("src=/dev/random,iommu=on,pci_segment=1,pci_device_id=7")?,
            RngConfig {
                src: PathBuf::from("/dev/random"),
                pci_common: PciDeviceCommonConfig {
                    id: None,
                    iommu: true,
                    pci_segment: 1,
                    pci_device_id: Some(7),
                },
            }
        );
        assert_eq!(
            RngConfig::parse("iommu=on")?,
            RngConfig {
                pci_common: PciDeviceCommonConfig {
                    id: None,
                    iommu: true,
                    pci_segment: 0,
                    pci_device_id: None,
                },
                ..Default::default()
            }
        );
        Ok(())
    }
}
