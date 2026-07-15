use std::sync::LazyLock;

use log::{debug, warn};
use option_parser::{IntegerList, OptionParser, StringList, Toggle};
use serde::{Deserialize, Serialize};

use crate::config::{Error, MAX_IOMMU_ADDRESS_WIDTH_BITS};
use crate::vm_config;
use crate::vm_config::{DEFAULT_IOMMU_ADDRESS_WIDTH_BITS, DEFAULT_NUM_PCI_SEGMENTS};

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlatformConfig {
    #[serde(default = "default_platformconfig_num_pci_segments")]
    pub num_pci_segments: u16,
    #[serde(default)]
    pub iommu_segments: Option<Box<[u16]>>,
    #[serde(default = "default_platformconfig_iommu_address_width_bits")]
    pub iommu_address_width_bits: u8,
    #[serde(default, alias = "serial_number")]
    pub system_serial_number: Option<String>,
    #[serde(default, alias = "uuid")]
    pub system_uuid: Option<String>,
    #[serde(default)]
    pub oem_strings: Option<Box<[String]>>,
    #[serde(default)]
    pub system_manufacturer: Option<String>,
    #[serde(default)]
    pub system_product_name: Option<String>,
    #[serde(default)]
    pub system_version: Option<String>,
    #[serde(default)]
    pub system_family: Option<String>,
    #[serde(default)]
    pub system_sku_number: Option<String>,
    #[serde(default)]
    pub chassis_asset_tag: Option<String>,
    #[cfg(feature = "tdx")]
    #[serde(default)]
    pub tdx: bool,
    #[cfg(feature = "sev_snp")]
    #[serde(default)]
    pub sev_snp: bool,
    #[serde(default)]
    pub iommufd: bool,
    // FDs are not serialized and any deserialized value is invalid; see NetConfig::fds.
    #[serde(default, deserialize_with = "deserialize_platformconfig_iommufd_fd")]
    pub iommufd_fd: Option<i32>,
    #[serde(default = "default_platformconfig_vfio_p2p_dma")]
    pub vfio_p2p_dma: bool,
}

impl From<PlatformConfig> for vm_config::PlatformConfig {
    fn from(value: PlatformConfig) -> Self {
        Self {
            num_pci_segments: value.num_pci_segments,
            iommu_segments: value.iommu_segments,
            iommu_address_width_bits: value.iommu_address_width_bits,
            system_serial_number: value.system_serial_number,
            system_uuid: value.system_uuid,
            oem_strings: value.oem_strings,
            system_manufacturer: value.system_manufacturer,
            system_product_name: value.system_product_name,
            system_version: value.system_version,
            system_family: value.system_family,
            system_sku_number: value.system_sku_number,
            chassis_asset_tag: value.chassis_asset_tag,
            #[cfg(feature = "tdx")]
            tdx: value.tdx,
            #[cfg(feature = "sev_snp")]
            sev_snp: value.sev_snp,
            iommufd: value.iommufd,
            iommufd_fd: value.iommufd_fd,
            vfio_p2p_dma: value.vfio_p2p_dma,
        }
    }
}

impl From<&vm_config::PlatformConfig> for PlatformConfig {
    fn from(value: &vm_config::PlatformConfig) -> Self {
        Self {
            num_pci_segments: value.num_pci_segments,
            iommu_segments: value.iommu_segments.clone(),
            iommu_address_width_bits: value.iommu_address_width_bits,
            system_serial_number: value.system_serial_number.clone(),
            system_uuid: value.system_uuid.clone(),
            oem_strings: value.oem_strings.clone(),
            system_manufacturer: value.system_manufacturer.clone(),
            system_product_name: value.system_product_name.clone(),
            system_version: value.system_version.clone(),
            system_family: value.system_family.clone(),
            system_sku_number: value.system_sku_number.clone(),
            chassis_asset_tag: value.chassis_asset_tag.clone(),
            #[cfg(feature = "tdx")]
            tdx: value.tdx,
            #[cfg(feature = "sev_snp")]
            sev_snp: value.sev_snp,
            iommufd: value.iommufd,
            iommufd_fd: value.iommufd_fd,
            vfio_p2p_dma: value.vfio_p2p_dma,
        }
    }
}

impl PlatformConfig {
    pub fn syntax() -> &'static str {
        static SYNTAX: LazyLock<String> = LazyLock::new(|| {
            let mut syntax = "Platform configuration parameters \
            \"num_pci_segments=<num_pci_segments>,iommu_segments=<list_of_segments>,\
            iommu_address_width=<bits>,iommufd=on|off,iommufd_fd=<fd>,vfio_p2p_dma=on|off,\
            system_manufacturer=<dmi_system_manufacturer>,\
            system_product_name=<dmi_system_product_name>,system_version=<dmi_system_version>,\
            system_serial_number=<dmi_system_serial_number>,system_uuid=<dmi_system_uuid>,\
            system_sku_number=<dmi_system_sku_number>,system_family=<dmi_system_family>,\
            oem_strings=<list_of_strings>,chassis_asset_tag=<dmi_chassis_asset_tag>"
                .to_string();

            if cfg!(feature = "tdx") {
                syntax.push_str(",tdx=on|off");
            }

            if cfg!(feature = "sev_snp") {
                syntax.push_str(",sev_snp=on|off");
            }

            syntax.push('"');

            syntax
        });

        &SYNTAX
    }

    pub fn parse(platform: &str) -> Result<Self, Error> {
        struct StringField {
            key: &'static str,
            apply: fn(&mut PlatformConfig, String),
        }

        const SMBIOS_STRING_FIELDS: &[StringField] = &[
            StringField {
                key: "system_manufacturer",
                apply: |config, value| config.system_manufacturer = Some(value),
            },
            StringField {
                key: "system_product_name",
                apply: |config, value| config.system_product_name = Some(value),
            },
            StringField {
                key: "system_version",
                apply: |config, value| config.system_version = Some(value),
            },
            StringField {
                key: "system_serial_number",
                apply: |config, value| config.system_serial_number = Some(value),
            },
            StringField {
                key: "system_uuid",
                apply: |config, value| config.system_uuid = Some(value),
            },
            StringField {
                key: "system_sku_number",
                apply: |config, value| config.system_sku_number = Some(value),
            },
            StringField {
                key: "system_family",
                apply: |config, value| config.system_family = Some(value),
            },
            StringField {
                key: "chassis_asset_tag",
                apply: |config, value| config.chassis_asset_tag = Some(value),
            },
        ];

        let mut parser = OptionParser::new();
        parser
            .add("num_pci_segments")
            .add("iommu_segments")
            .add("iommu_address_width")
            .add("serial_number")
            .add("uuid")
            .add("oem_strings")
            .add("iommufd")
            .add("iommufd_fd")
            .add("vfio_p2p_dma");
        for field in SMBIOS_STRING_FIELDS {
            parser.add(field.key);
        }
        #[cfg(feature = "tdx")]
        parser.add("tdx");
        #[cfg(feature = "sev_snp")]
        parser.add("sev_snp");
        parser.parse(platform).map_err(Error::ParsePlatform)?;

        let num_pci_segments: u16 = parser
            .convert("num_pci_segments")
            .map_err(Error::ParsePlatform)?
            .unwrap_or(DEFAULT_NUM_PCI_SEGMENTS);
        let iommu_segments = parser
            .convert::<IntegerList>("iommu_segments")
            .map_err(Error::ParsePlatform)?
            .map(|v| v.0.iter().map(|e| *e as u16).collect());
        let iommu_address_width_bits: u8 = parser
            .convert("iommu_address_width")
            .map_err(Error::ParsePlatform)?
            .unwrap_or(MAX_IOMMU_ADDRESS_WIDTH_BITS);
        let oem_strings = parser
            .convert::<StringList>("oem_strings")
            .map_err(Error::ParsePlatform)?
            .map(|v| v.0.into_boxed_slice());
        let iommufd_fd = parser
            .convert::<i32>("iommufd_fd")
            .map_err(Error::ParsePlatform)?;
        // `iommufd_fd=<n>` implies `iommufd=on` unless the user explicitly set the value.
        let iommufd = parser
            .convert::<Toggle>("iommufd")
            .map_err(Error::ParsePlatform)?
            .map_or(iommufd_fd.is_some(), |Toggle(v)| v);
        let vfio_p2p_dma = parser
            .convert::<Toggle>("vfio_p2p_dma")
            .map_err(Error::ParsePlatform)?
            .unwrap_or(Toggle(true))
            .0;
        #[cfg(feature = "tdx")]
        let tdx = parser
            .convert::<Toggle>("tdx")
            .map_err(Error::ParsePlatform)?
            .unwrap_or(Toggle(false))
            .0;
        #[cfg(feature = "sev_snp")]
        let sev_snp = parser
            .convert::<Toggle>("sev_snp")
            .map_err(Error::ParsePlatform)?
            .unwrap_or(Toggle(false))
            .0;

        let mut platform_config = PlatformConfig {
            num_pci_segments,
            iommu_segments,
            iommu_address_width_bits,
            system_serial_number: None,
            system_uuid: None,
            oem_strings,
            system_manufacturer: None,
            system_product_name: None,
            system_version: None,
            system_family: None,
            system_sku_number: None,
            chassis_asset_tag: None,
            iommufd,
            iommufd_fd,
            #[cfg(feature = "tdx")]
            tdx,
            #[cfg(feature = "sev_snp")]
            sev_snp,
            vfio_p2p_dma,
        };

        for field in SMBIOS_STRING_FIELDS {
            if let Some(value) = parser
                .convert::<String>(field.key)
                .map_err(Error::ParsePlatform)?
            {
                (field.apply)(&mut platform_config, value);
            }
        }

        let legacy_serial_number = parser
            .convert::<String>("serial_number")
            .map_err(Error::ParsePlatform)?;
        if legacy_serial_number.is_some() {
            warn!("'serial_number' in --platform is deprecated; use 'system_serial_number'.");
        }
        platform_config.system_serial_number = platform_config
            .system_serial_number
            .or(legacy_serial_number);

        let legacy_uuid = parser
            .convert::<String>("uuid")
            .map_err(Error::ParsePlatform)?;
        if legacy_uuid.is_some() {
            warn!("'uuid' in --platform is deprecated; use 'system_uuid'.");
        }
        platform_config.system_uuid = platform_config.system_uuid.or(legacy_uuid);

        Ok(platform_config)
    }
}

pub fn default_platformconfig_num_pci_segments() -> u16 {
    DEFAULT_NUM_PCI_SEGMENTS
}

pub fn default_platformconfig_iommu_address_width_bits() -> u8 {
    DEFAULT_IOMMU_ADDRESS_WIDTH_BITS
}

pub fn default_platformconfig_vfio_p2p_dma() -> bool {
    true
}

fn deserialize_platformconfig_iommufd_fd<'de, D>(d: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let invalid_fd: Option<i32> = Option::deserialize(d)?;
    if invalid_fd.is_some() {
        debug!(
            "FD in 'PlatformConfig::iommufd_fd' won't be deserialized as it is most likely invalid now. Deserializing it as -1."
        );
        Ok(Some(-1))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_iommufd_fd_parsing() -> Result<(), Error> {
        // `iommufd_fd=N` alone implies `iommufd=on`.
        let p = PlatformConfig::parse("iommufd_fd=42")?;
        assert!(p.iommufd);
        assert_eq!(p.iommufd_fd, Some(42));

        // Explicit `iommufd=on,iommufd_fd=N` is the same.
        let p = PlatformConfig::parse("iommufd=on,iommufd_fd=42")?;
        assert!(p.iommufd);
        assert_eq!(p.iommufd_fd, Some(42));

        // No flags → both default to off.
        let p = PlatformConfig::parse("")?;
        assert!(!p.iommufd);
        assert_eq!(p.iommufd_fd, None);

        Ok(())
    }
}
