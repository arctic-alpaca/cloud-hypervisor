use std::path::PathBuf;
use std::result;
use std::str::FromStr;

use option_parser::{ByteSized, OptionParser, Toggle};
use serde::{Deserialize, Serialize};

use crate::config::Error;
use crate::vm_config;
use crate::vm_config::DEFAULT_MEMORY_MB;

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct MemoryConfig {
    pub size: u64,
    #[serde(default)]
    pub mergeable: bool,
    #[serde(default)]
    pub hotplug_method: HotplugMethod,
    #[serde(default)]
    pub hotplug_size: Option<u64>,
    #[serde(default)]
    pub hotplugged_size: Option<u64>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub hugepages: bool,
    #[serde(default)]
    pub hugepage_size: Option<u64>,
    #[serde(default)]
    pub prefault: bool,
    #[serde(default)]
    pub reserve: bool,
    #[serde(default)]
    pub zones: Option<Vec<MemoryZoneConfig>>,
    #[serde(default = "default_memoryconfig_thp")]
    pub thp: bool,
}

impl From<MemoryConfig> for vm_config::MemoryConfig {
    fn from(value: MemoryConfig) -> Self {
        Self {
            size: value.size,
            mergeable: value.mergeable,
            hotplug_method: value.hotplug_method.into(),
            hotplug_size: value.hotplug_size,
            hotplugged_size: value.hotplugged_size,
            shared: value.shared,
            hugepages: value.hugepages,
            hugepage_size: value.hugepage_size,
            prefault: value.prefault,
            reserve: value.reserve,
            zones: value
                .zones
                .map(|zones| zones.into_iter().map(Into::into).collect()),
            thp: value.thp,
        }
    }
}

impl From<&vm_config::MemoryConfig> for MemoryConfig {
    fn from(value: &vm_config::MemoryConfig) -> Self {
        Self {
            size: value.size,
            mergeable: value.mergeable,
            hotplug_method: (&value.hotplug_method).into(),
            hotplug_size: value.hotplug_size,
            hotplugged_size: value.hotplugged_size,
            shared: value.shared,
            hugepages: value.hugepages,
            hugepage_size: value.hugepage_size,
            prefault: value.prefault,
            reserve: value.reserve,
            zones: value
                .zones
                .as_ref()
                .map(|zones| zones.iter().map(Into::into).collect()),
            thp: value.thp,
        }
    }
}

impl MemoryConfig {
    #[expect(clippy::needless_pass_by_value)]
    pub fn parse(memory: &str, memory_zones: Option<Vec<&str>>) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("size")
            .add("file")
            .add("mergeable")
            .add("hotplug_method")
            .add("hotplug_size")
            .add("hotplugged_size")
            .add("shared")
            .add("hugepages")
            .add("hugepage_size")
            .add("prefault")
            .add("reserve")
            .add("thp");
        parser.parse(memory).map_err(Error::ParseMemory)?;

        let size = parser
            .convert::<ByteSized>("size")
            .map_err(Error::ParseMemory)?
            .unwrap_or(ByteSized(DEFAULT_MEMORY_MB << 20))
            .0;
        let mergeable = parser
            .convert::<Toggle>("mergeable")
            .map_err(Error::ParseMemory)?
            .unwrap_or(Toggle(false))
            .0;
        let hotplug_method = parser
            .convert("hotplug_method")
            .map_err(Error::ParseMemory)?
            .unwrap_or_default();
        let hotplug_size = parser
            .convert::<ByteSized>("hotplug_size")
            .map_err(Error::ParseMemory)?
            .map(|v| v.0);
        let hotplugged_size = parser
            .convert::<ByteSized>("hotplugged_size")
            .map_err(Error::ParseMemory)?
            .map(|v| v.0);
        let shared = parser
            .convert::<Toggle>("shared")
            .map_err(Error::ParseMemory)?
            .unwrap_or(Toggle(false))
            .0;
        let hugepages = parser
            .convert::<Toggle>("hugepages")
            .map_err(Error::ParseMemory)?
            .unwrap_or(Toggle(false))
            .0;
        let hugepage_size = parser
            .convert::<ByteSized>("hugepage_size")
            .map_err(Error::ParseMemory)?
            .map(|v| v.0);
        let prefault = parser
            .convert::<Toggle>("prefault")
            .map_err(Error::ParseMemory)?
            .unwrap_or(Toggle(false))
            .0;
        let reserve = parser
            .convert::<Toggle>("reserve")
            .map_err(Error::ParseMemory)?
            .unwrap_or(Toggle(false))
            .0;
        let thp = parser
            .convert::<Toggle>("thp")
            .map_err(Error::ParseMemory)?
            .unwrap_or(Toggle(true))
            .0;

        let zones: Option<Vec<MemoryZoneConfig>> = if let Some(memory_zones) = &memory_zones {
            let mut zones = Vec::new();
            for memory_zone in memory_zones.iter() {
                let mut parser = OptionParser::new();
                parser
                    .add("id")
                    .add("size")
                    .add("file")
                    .add("shared")
                    .add("hugepages")
                    .add("hugepage_size")
                    .add("host_numa_node")
                    .add("hotplug_size")
                    .add("hotplugged_size")
                    .add("prefault")
                    .add("reserve")
                    .add("mergeable");
                parser.parse(memory_zone).map_err(Error::ParseMemoryZone)?;

                let id = parser.get("id").ok_or(Error::ParseMemoryZoneIdMissing)?;
                let size = parser
                    .convert::<ByteSized>("size")
                    .map_err(Error::ParseMemoryZone)?
                    .unwrap_or(ByteSized(DEFAULT_MEMORY_MB << 20))
                    .0;
                let file = parser.get("file").map(PathBuf::from);
                let shared = parser
                    .convert::<Toggle>("shared")
                    .map_err(Error::ParseMemoryZone)?
                    .unwrap_or(Toggle(false))
                    .0;
                let hugepages = parser
                    .convert::<Toggle>("hugepages")
                    .map_err(Error::ParseMemoryZone)?
                    .unwrap_or(Toggle(false))
                    .0;
                let hugepage_size = parser
                    .convert::<ByteSized>("hugepage_size")
                    .map_err(Error::ParseMemoryZone)?
                    .map(|v| v.0);

                let host_numa_node = parser
                    .convert::<u32>("host_numa_node")
                    .map_err(Error::ParseMemoryZone)?;
                let hotplug_size = parser
                    .convert::<ByteSized>("hotplug_size")
                    .map_err(Error::ParseMemoryZone)?
                    .map(|v| v.0);
                let hotplugged_size = parser
                    .convert::<ByteSized>("hotplugged_size")
                    .map_err(Error::ParseMemoryZone)?
                    .map(|v| v.0);
                let prefault = parser
                    .convert::<Toggle>("prefault")
                    .map_err(Error::ParseMemoryZone)?
                    .unwrap_or(Toggle(false))
                    .0;
                let reserve = parser
                    .convert::<Toggle>("reserve")
                    .map_err(Error::ParseMemoryZone)?
                    .unwrap_or(Toggle(false))
                    .0;
                let mergeable = parser
                    .convert::<Toggle>("mergeable")
                    .map_err(Error::ParseMemoryZone)?
                    .unwrap_or(Toggle(mergeable))
                    .0;

                zones.push(MemoryZoneConfig {
                    id,
                    size,
                    file,
                    shared,
                    hugepages,
                    hugepage_size,
                    host_numa_node,
                    hotplug_size,
                    hotplugged_size,
                    prefault,
                    reserve,
                    mergeable,
                });
            }
            Some(zones)
        } else {
            None
        };

        Ok(MemoryConfig {
            size,
            mergeable,
            hotplug_method,
            hotplug_size,
            hotplugged_size,
            shared,
            hugepages,
            hugepage_size,
            prefault,
            reserve,
            zones,
            thp,
        })
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        MemoryConfig {
            size: DEFAULT_MEMORY_MB << 20,
            mergeable: false,
            hotplug_method: HotplugMethod::Acpi,
            hotplug_size: None,
            hotplugged_size: None,
            shared: false,
            hugepages: false,
            hugepage_size: None,
            prefault: false,
            reserve: false,
            zones: None,
            thp: true,
        }
    }
}

fn default_memoryconfig_thp() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum HotplugMethod {
    #[default]
    Acpi,
    VirtioMem,
}

#[derive(Debug)]
pub enum ParseHotplugMethodError {
    InvalidValue(String),
}

impl FromStr for HotplugMethod {
    type Err = ParseHotplugMethodError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "acpi" => Ok(HotplugMethod::Acpi),
            "virtio-mem" => Ok(HotplugMethod::VirtioMem),
            _ => Err(ParseHotplugMethodError::InvalidValue(s.to_owned())),
        }
    }
}

impl From<HotplugMethod> for vm_config::HotplugMethod {
    fn from(value: HotplugMethod) -> Self {
        match value {
            HotplugMethod::Acpi => Self::Acpi,
            HotplugMethod::VirtioMem => Self::VirtioMem,
        }
    }
}

impl From<&vm_config::HotplugMethod> for HotplugMethod {
    fn from(value: &vm_config::HotplugMethod) -> Self {
        match value {
            vm_config::HotplugMethod::Acpi => Self::Acpi,
            vm_config::HotplugMethod::VirtioMem => Self::VirtioMem,
        }
    }
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct MemoryZoneConfig {
    pub id: String,
    pub size: u64,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub hugepages: bool,
    #[serde(default)]
    pub hugepage_size: Option<u64>,
    #[serde(default)]
    pub host_numa_node: Option<u32>,
    #[serde(default)]
    pub hotplug_size: Option<u64>,
    #[serde(default)]
    pub hotplugged_size: Option<u64>,
    #[serde(default)]
    pub prefault: bool,
    #[serde(default)]
    pub reserve: bool,
    #[serde(default)]
    pub mergeable: bool,
}

impl From<MemoryZoneConfig> for vm_config::MemoryZoneConfig {
    fn from(value: MemoryZoneConfig) -> Self {
        Self {
            id: value.id,
            size: value.size,
            file: value.file,
            shared: value.shared,
            hugepages: value.hugepages,
            hugepage_size: value.hugepage_size,
            host_numa_node: value.host_numa_node,
            hotplug_size: value.hotplug_size,
            hotplugged_size: value.hotplugged_size,
            prefault: value.prefault,
            reserve: value.reserve,
            mergeable: value.mergeable,
        }
    }
}

impl From<&vm_config::MemoryZoneConfig> for MemoryZoneConfig {
    fn from(value: &vm_config::MemoryZoneConfig) -> Self {
        Self {
            id: value.id.clone(),
            size: value.size,
            file: value.file.clone(),
            shared: value.shared,
            hugepages: value.hugepages,
            hugepage_size: value.hugepage_size,
            host_numa_node: value.host_numa_node,
            hotplug_size: value.hotplug_size,
            hotplugged_size: value.hotplugged_size,
            prefault: value.prefault,
            reserve: value.reserve,
            mergeable: value.mergeable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mem_zone_parsing() -> Result<(), Error> {
        // mergeable defaults to false
        assert_eq!(
            MemoryConfig::parse("size=0", Some(vec!["id=mem0,size=1G"]))?,
            MemoryConfig {
                size: 0,
                zones: Some(vec![MemoryZoneConfig {
                    id: "mem0".to_string(),
                    size: 1 << 30,
                    ..Default::default()
                }]),
                ..Default::default()
            }
        );
        // mergeable=on
        assert_eq!(
            MemoryConfig::parse("size=0", Some(vec!["id=mem0,size=1G,mergeable=on"]))?,
            MemoryConfig {
                size: 0,
                zones: Some(vec![MemoryZoneConfig {
                    id: "mem0".to_string(),
                    size: 1 << 30,
                    mergeable: true,
                    ..Default::default()
                }]),
                ..Default::default()
            }
        );
        // mergeable=off is explicit false
        assert_eq!(
            MemoryConfig::parse("size=0", Some(vec!["id=mem0,size=1G,mergeable=off"]))?,
            MemoryConfig {
                size: 0,
                zones: Some(vec![MemoryZoneConfig {
                    id: "mem0".to_string(),
                    size: 1 << 30,
                    mergeable: false,
                    ..Default::default()
                }]),
                ..Default::default()
            }
        );
        // per-zone mergeable independent of global mergeable
        assert_eq!(
            MemoryConfig::parse(
                "size=1G,mergeable=off",
                Some(vec!["id=hotplug,size=0,hotplug_size=4G,mergeable=on"])
            )?,
            MemoryConfig {
                size: 1 << 30,
                mergeable: false,
                hotplug_method: HotplugMethod::Acpi,
                zones: Some(vec![MemoryZoneConfig {
                    id: "hotplug".to_string(),
                    size: 0,
                    hotplug_size: Some(4 << 30),
                    mergeable: true,
                    ..Default::default()
                }]),
                ..Default::default()
            }
        );
        // global mergeable=on inherited by zone with no explicit mergeable
        assert_eq!(
            MemoryConfig::parse("size=0,mergeable=on", Some(vec!["id=mem0,size=1G"]))?,
            MemoryConfig {
                size: 0,
                mergeable: true,
                zones: Some(vec![MemoryZoneConfig {
                    id: "mem0".to_string(),
                    size: 1 << 30,
                    mergeable: true,
                    ..Default::default()
                }]),
                ..Default::default()
            }
        );
        // reserve=on on a zone
        assert_eq!(
            MemoryConfig::parse("size=0", Some(vec!["id=mem0,size=1G,reserve=on"]))?,
            MemoryConfig {
                size: 0,
                zones: Some(vec![MemoryZoneConfig {
                    id: "mem0".to_string(),
                    size: 1 << 30,
                    reserve: true,
                    ..Default::default()
                }]),
                ..Default::default()
            }
        );
        Ok(())
    }

    #[test]
    fn test_mem_parsing() -> Result<(), Error> {
        assert_eq!(MemoryConfig::parse("", None)?, MemoryConfig::default());
        // Default string
        assert_eq!(
            MemoryConfig::parse("size=512M", None)?,
            MemoryConfig::default()
        );
        assert_eq!(
            MemoryConfig::parse("size=512M,mergeable=on", None)?,
            MemoryConfig {
                size: 512 << 20,
                mergeable: true,
                ..Default::default()
            }
        );
        assert_eq!(
            MemoryConfig::parse("mergeable=on", None)?,
            MemoryConfig {
                mergeable: true,
                ..Default::default()
            }
        );
        assert_eq!(
            MemoryConfig::parse("size=1G,mergeable=off", None)?,
            MemoryConfig {
                size: 1 << 30,
                mergeable: false,
                ..Default::default()
            }
        );
        assert_eq!(
            MemoryConfig::parse("hotplug_method=acpi", None)?,
            MemoryConfig {
                ..Default::default()
            }
        );
        assert_eq!(
            MemoryConfig::parse("hotplug_method=acpi,hotplug_size=512M", None)?,
            MemoryConfig {
                hotplug_size: Some(512 << 20),
                ..Default::default()
            }
        );
        assert_eq!(
            MemoryConfig::parse("hotplug_method=virtio-mem,hotplug_size=512M", None)?,
            MemoryConfig {
                hotplug_size: Some(512 << 20),
                hotplug_method: HotplugMethod::VirtioMem,
                ..Default::default()
            }
        );
        assert_eq!(
            MemoryConfig::parse("hugepages=on,size=1G,hugepage_size=2M", None)?,
            MemoryConfig {
                hugepage_size: Some(2 << 20),
                size: 1 << 30,
                hugepages: true,
                ..Default::default()
            }
        );
        // reserve=on opts out of MAP_NORESERVE
        assert_eq!(
            MemoryConfig::parse("size=1G,hugepages=on,reserve=on", None)?,
            MemoryConfig {
                size: 1 << 30,
                hugepages: true,
                reserve: true,
                ..Default::default()
            }
        );
        Ok(())
    }
}
