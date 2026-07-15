use std::path::PathBuf;
#[cfg(feature = "fw_cfg")]
use std::str::FromStr;

#[cfg(feature = "fw_cfg")]
use option_parser::{OptionParser, OptionParserError, Toggle};
use serde::{Deserialize, Serialize};

#[cfg(feature = "fw_cfg")]
use crate::config::Error;
use crate::vm_config;

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PayloadConfig {
    #[serde(default)]
    pub firmware: Option<PathBuf>,
    #[serde(default)]
    pub kernel: Option<PathBuf>,
    #[serde(default)]
    pub cmdline: Option<String>,
    #[serde(default)]
    pub initramfs: Option<PathBuf>,
    #[cfg(feature = "igvm")]
    #[serde(default)]
    pub igvm: Option<PathBuf>,
    #[cfg(feature = "sev_snp")]
    #[serde(default)]
    pub host_data: Option<String>,
    #[cfg(feature = "fw_cfg")]
    pub fw_cfg_config: Option<FwCfgConfig>,
}

impl From<PayloadConfig> for vm_config::PayloadConfig {
    fn from(value: PayloadConfig) -> Self {
        Self {
            firmware: value.firmware,
            kernel: value.kernel,
            cmdline: value.cmdline,
            initramfs: value.initramfs,
            #[cfg(feature = "igvm")]
            igvm: value.igvm,
            #[cfg(feature = "sev_snp")]
            host_data: value.host_data,
            #[cfg(feature = "fw_cfg")]
            fw_cfg_config: value.fw_cfg_config.map(Into::into),
        }
    }
}

impl From<&vm_config::PayloadConfig> for PayloadConfig {
    fn from(value: &vm_config::PayloadConfig) -> Self {
        Self {
            firmware: value.firmware.clone(),
            kernel: value.kernel.clone(),
            cmdline: value.cmdline.clone(),
            initramfs: value.initramfs.clone(),
            #[cfg(feature = "igvm")]
            igvm: value.igvm.clone(),
            #[cfg(feature = "sev_snp")]
            host_data: value.host_data.clone(),
            #[cfg(feature = "fw_cfg")]
            fw_cfg_config: value.fw_cfg_config.as_ref().map(Into::into),
        }
    }
}

#[cfg(feature = "fw_cfg")]
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct FwCfgConfig {
    pub e820: bool,
    pub kernel: bool,
    pub cmdline: bool,
    pub initramfs: bool,
    pub acpi_tables: bool,
    pub items: Option<FwCfgItemList>,
}

#[cfg(feature = "fw_cfg")]
impl From<FwCfgConfig> for vm_config::FwCfgConfig {
    fn from(value: FwCfgConfig) -> Self {
        Self {
            e820: value.e820,
            kernel: value.kernel,
            cmdline: value.cmdline,
            initramfs: value.initramfs,
            acpi_tables: value.acpi_tables,
            items: value.items.map(Into::into),
        }
    }
}

#[cfg(feature = "fw_cfg")]
impl From<&vm_config::FwCfgConfig> for FwCfgConfig {
    fn from(value: &vm_config::FwCfgConfig) -> Self {
        Self {
            e820: value.e820,
            kernel: value.kernel,
            cmdline: value.cmdline,
            initramfs: value.initramfs,
            acpi_tables: value.acpi_tables,
            items: value.items.as_ref().map(Into::into),
        }
    }
}

#[cfg(feature = "fw_cfg")]
impl FwCfgConfig {
    pub const SYNTAX: &'static str = "Boot params to pass to FW CFG device \
    \"e820=on|off,kernel=on|off,cmdline=on|off,initramfs=on|off,acpi_table=on|off, \
    items=[name=<item_name>,file=<file_path>:name=<item_name>,string=<string_value>]\"";

    pub fn parse(fw_cfg_config: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("e820")
            .add("kernel")
            .add("cmdline")
            .add("initramfs")
            .add("acpi_table")
            .add("items");
        parser.parse(fw_cfg_config).map_err(Error::ParseFwCfgItem)?;
        let e820 = parser
            .convert::<Toggle>("e820")
            .map_err(Error::ParseFwCfgItem)?
            .unwrap_or(Toggle(true))
            .0;
        let kernel = parser
            .convert::<Toggle>("kernel")
            .map_err(Error::ParseFwCfgItem)?
            .unwrap_or(Toggle(true))
            .0;
        let cmdline = parser
            .convert::<Toggle>("cmdline")
            .map_err(Error::ParseFwCfgItem)?
            .unwrap_or(Toggle(true))
            .0;
        let initramfs = parser
            .convert::<Toggle>("initramfs")
            .map_err(Error::ParseFwCfgItem)?
            .unwrap_or(Toggle(true))
            .0;
        let acpi_tables = parser
            .convert::<Toggle>("acpi_table")
            .map_err(Error::ParseFwCfgItem)?
            .unwrap_or(Toggle(true))
            .0;
        let items = if parser.is_set("items") {
            Some(
                parser
                    .convert::<FwCfgItemList>("items")
                    .map_err(Error::ParseFwCfgItem)?
                    .unwrap(),
            )
        } else {
            None
        };

        Ok(FwCfgConfig {
            e820,
            kernel,
            cmdline,
            initramfs,
            acpi_tables,
            items,
        })
    }
}

#[cfg(feature = "fw_cfg")]
impl Default for FwCfgConfig {
    fn default() -> Self {
        FwCfgConfig {
            e820: true,
            kernel: true,
            cmdline: true,
            initramfs: true,
            acpi_tables: true,
            items: None,
        }
    }
}

#[cfg(feature = "fw_cfg")]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FwCfgItemList {
    #[serde(default)]
    pub item_list: Vec<FwCfgItem>,
}

#[cfg(feature = "fw_cfg")]
impl From<FwCfgItemList> for vm_config::FwCfgItemList {
    fn from(value: FwCfgItemList) -> Self {
        Self {
            item_list: value.item_list.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "fw_cfg")]
impl From<&vm_config::FwCfgItemList> for FwCfgItemList {
    fn from(value: &vm_config::FwCfgItemList) -> Self {
        Self {
            item_list: value.item_list.iter().map(Into::into).collect(),
        }
    }
}

#[cfg(feature = "fw_cfg")]
pub enum FwCfgItemError {
    InvalidValue(String),
}

#[cfg(feature = "fw_cfg")]
impl FromStr for FwCfgItemList {
    type Err = FwCfgItemError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let body = s
            .trim()
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(|| FwCfgItemError::InvalidValue(s.to_string()))?;

        let mut fw_cfg_items: Vec<FwCfgItem> = vec![];
        let items: Vec<&str> = body.split(':').collect();
        for item in items {
            fw_cfg_items.push(
                FwCfgItem::parse(item)
                    .map_err(|_| FwCfgItemError::InvalidValue(item.to_string()))?,
            );
        }
        Ok(FwCfgItemList {
            item_list: fw_cfg_items,
        })
    }
}

#[cfg(feature = "fw_cfg")]
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FwCfgItem {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub string: Option<String>,
}

#[cfg(feature = "fw_cfg")]
impl From<FwCfgItem> for vm_config::FwCfgItem {
    fn from(value: FwCfgItem) -> Self {
        Self {
            name: value.name,
            file: value.file,
            string: value.string,
        }
    }
}

#[cfg(feature = "fw_cfg")]
impl From<&vm_config::FwCfgItem> for FwCfgItem {
    fn from(value: &vm_config::FwCfgItem) -> Self {
        Self {
            name: value.name.clone(),
            file: value.file.clone(),
            string: value.string.clone(),
        }
    }
}

#[cfg(feature = "fw_cfg")]
impl FwCfgItem {
    pub fn parse(fw_cfg: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add("name").add("file").add("string");
        parser.parse(fw_cfg).map_err(Error::ParseFwCfgItem)?;

        let name =
            parser
                .get("name")
                .ok_or(Error::ParseFwCfgItem(OptionParserError::InvalidValue(
                    "missing FwCfgItem name".to_string(),
                )))?;
        let file = parser.get("file").map(PathBuf::from);
        let string = parser.get("string");
        Ok(FwCfgItem { name, file, string })
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "fw_cfg")]
    use super::*;

    #[test]
    #[cfg(feature = "fw_cfg")]
    fn test_fw_cfg_config_item_list_parsing() -> Result<(), Error> {
        // Empty list
        FwCfgConfig::parse("items=[]").unwrap_err();
        // Missing closing bracket
        FwCfgConfig::parse("items=[name=opt/org.test/fw_cfg_test_item,file=/tmp/fw_cfg_test_item")
            .unwrap_err();
        // Single file Item
        assert_eq!(
            FwCfgConfig::parse(
                "items=[name=opt/org.test/fw_cfg_test_item,file=/tmp/fw_cfg_test_item]"
            )?,
            FwCfgConfig {
                items: Some(FwCfgItemList {
                    item_list: vec![FwCfgItem {
                        name: "opt/org.test/fw_cfg_test_item".to_string(),
                        file: Some(PathBuf::from("/tmp/fw_cfg_test_item")),
                        string: None,
                    }]
                }),
                ..Default::default()
            },
        );
        // Multiple file Items
        assert_eq!(
            FwCfgConfig::parse(
                "items=[name=opt/org.test/fw_cfg_test_item,file=/tmp/fw_cfg_test_item:name=opt/org.test/fw_cfg_test_item2,file=/tmp/fw_cfg_test_item2]"
            )?,
            FwCfgConfig {
                items: Some(FwCfgItemList {
                    item_list: vec![
                        FwCfgItem {
                            name: "opt/org.test/fw_cfg_test_item".to_string(),
                            file: Some(PathBuf::from("/tmp/fw_cfg_test_item")),
                            string: None,
                        },
                        FwCfgItem {
                            name: "opt/org.test/fw_cfg_test_item2".to_string(),
                            file: Some(PathBuf::from("/tmp/fw_cfg_test_item2")),
                            string: None,
                        }
                    ]
                }),
                ..Default::default()
            },
        );
        // Single string Item (for OVMF MMIO64 config, GPU CC passthrough, etc.)
        assert_eq!(
            FwCfgConfig::parse("items=[name=opt/ovmf/X-PciMmio64Mb,string=262144]")?,
            FwCfgConfig {
                items: Some(FwCfgItemList {
                    item_list: vec![FwCfgItem {
                        name: "opt/ovmf/X-PciMmio64Mb".to_string(),
                        file: None,
                        string: Some("262144".to_string()),
                    }]
                }),
                ..Default::default()
            },
        );
        // Mixed file and string Items
        assert_eq!(
            FwCfgConfig::parse(
                "items=[name=opt/org.test/fw_cfg_test_item,file=/tmp/fw_cfg_test_item:name=opt/ovmf/X-PciMmio64Mb,string=262144]"
            )?,
            FwCfgConfig {
                items: Some(FwCfgItemList {
                    item_list: vec![
                        FwCfgItem {
                            name: "opt/org.test/fw_cfg_test_item".to_string(),
                            file: Some(PathBuf::from("/tmp/fw_cfg_test_item")),
                            string: None,
                        },
                        FwCfgItem {
                            name: "opt/ovmf/X-PciMmio64Mb".to_string(),
                            file: None,
                            string: Some("262144".to_string()),
                        }
                    ]
                }),
                ..Default::default()
            },
        );
        // Missing both file and string parses OK but fails validation
        let missing_content =
            FwCfgConfig::parse("items=[name=opt/org.test/missing_content]").unwrap();
        assert_eq!(
            missing_content.items.as_ref().unwrap().item_list[0].file,
            None
        );
        assert_eq!(
            missing_content.items.as_ref().unwrap().item_list[0].string,
            None
        );
        // Both file and string parses OK but fails validation
        let both = FwCfgConfig::parse("items=[name=opt/org.test/both,file=/tmp/test,string=test]")
            .unwrap();
        assert!(both.items.as_ref().unwrap().item_list[0].file.is_some());
        assert!(both.items.as_ref().unwrap().item_list[0].string.is_some());
        Ok(())
    }
}
