use std::path::PathBuf;
use std::result;
use std::str::FromStr;

use log::debug;
use option_parser::{OptionParser, OptionParserError, Toggle, Tuple, TupleList};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config;

#[derive(Debug, Error)]
pub enum ParseRestoreError {
    #[error("Error parsing --restore")]
    ParseRestore(#[source] OptionParserError),
    /// Missing restore source_url parameter.
    #[error("Error parsing --restore: source_url missing")]
    ParseRestoreSourceUrlMissing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum MemoryRestoreMode {
    /// Restore by eagerly copying the snapshot into guest RAM before resume.
    #[default]
    Copy,
    /// Restore lazily by faulting snapshot pages into guest RAM on demand.
    OnDemand,
}

impl From<MemoryRestoreMode> for config::MemoryRestoreMode {
    fn from(value: MemoryRestoreMode) -> Self {
        match value {
            MemoryRestoreMode::Copy => Self::Copy,
            MemoryRestoreMode::OnDemand => Self::OnDemand,
        }
    }
}

#[derive(Debug, Error)]
pub enum MemoryRestoreModeParseError {
    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

impl FromStr for MemoryRestoreMode {
    type Err = MemoryRestoreModeParseError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "copy" => Ok(Self::Copy),
            "ondemand" => Ok(Self::OnDemand),
            _ => Err(MemoryRestoreModeParseError::InvalidValue(s.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct RestoreConfig {
    pub source_url: PathBuf,
    #[serde(default)]
    pub prefault: bool,
    #[serde(default)]
    pub memory_restore_mode: MemoryRestoreMode,
    #[serde(default)]
    pub net_fds: Option<Vec<RestoredNetConfig>>,
    #[serde(default)]
    pub resume: bool,
}

impl RestoreConfig {
    pub const SYNTAX: &'static str = "Restore from a VM snapshot. \
        \nRestore parameters \"source_url=<source_url>,prefault=on|off,memory_restore_mode=copy|ondemand,\
        net_fds=<list_of_net_ids_with_their_associated_fds>,resume=true|false\" \
        \n`source_url` should be a valid URL (e.g file:///foo/bar or tcp://192.168.1.10/foo) \
        \n`prefault` controls eager prefaulting for the copy-based restore path (disabled by default) \
        \n`memory_restore_mode=copy` preserves the existing eager read-copy restore behavior, while `memory_restore_mode=ondemand` enables lazy demand paging and fails restore if userfaultfd support is unavailable \
        \n`net_fds` is a list of net ids with new file descriptors. \
        Only net devices backed by FDs directly are needed as input.\
        \n `resume` controls whether the VM will be directly resumed after restore ";

    pub fn parse(restore: &str) -> Result<Self, ParseRestoreError> {
        let mut parser = OptionParser::new();
        parser
            .add("source_url")
            .add("prefault")
            .add("memory_restore_mode")
            .add("net_fds")
            .add("resume");
        parser
            .parse(restore)
            .map_err(ParseRestoreError::ParseRestore)?;

        let source_url = parser
            .get("source_url")
            .map(PathBuf::from)
            .ok_or(ParseRestoreError::ParseRestoreSourceUrlMissing)?;
        let prefault = parser
            .convert::<Toggle>("prefault")
            .map_err(ParseRestoreError::ParseRestore)?
            .unwrap_or(Toggle(false))
            .0;
        let memory_restore_mode = parser
            .convert::<MemoryRestoreMode>("memory_restore_mode")
            .map_err(ParseRestoreError::ParseRestore)?
            .unwrap_or_default();
        let net_fds = parser
            .convert::<TupleList<String, Vec<u64>>>("net_fds")
            .map_err(ParseRestoreError::ParseRestore)?
            .map(|v| {
                v.0.iter()
                    .map(|Tuple(id, fds)| RestoredNetConfig {
                        id: id.clone(),
                        num_fds: fds.len(),
                        fds: Some(fds.iter().map(|e| *e as i32).collect()),
                    })
                    .collect()
            });
        let resume = parser
            .convert::<Toggle>("resume")
            .map_err(ParseRestoreError::ParseRestore)?
            .unwrap_or(Toggle(false))
            .0;

        Ok(RestoreConfig {
            source_url,
            prefault,
            memory_restore_mode,
            net_fds,
            resume,
        })
    }
}

impl From<RestoreConfig> for config::RestoreConfig {
    fn from(value: RestoreConfig) -> Self {
        Self {
            source_url: value.source_url,
            prefault: value.prefault,
            memory_restore_mode: value.memory_restore_mode.into(),
            net_fds: value
                .net_fds
                .map(|net_fds| net_fds.into_iter().map(Into::into).collect()),
            resume: value.resume,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct RestoredNetConfig {
    pub id: String,
    #[serde(default)]
    pub num_fds: usize,
    // Special deserialize handling:
    // A serialize-deserialize cycle typically happens across processes.
    // Therefore, we don't serialize FDs, and whatever value is here after
    // deserialization is invalid.
    //
    // Valid FDs are transmitted via a different channel (SCM_RIGHTS message)
    // and will be populated into this struct on the destination VMM eventually.
    #[serde(default, deserialize_with = "deserialize_restorednetconfig_fds")]
    pub fds: Option<Vec<i32>>,
}

fn deserialize_restorednetconfig_fds<'de, D>(d: D) -> result::Result<Option<Vec<i32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let invalid_fds: Option<Vec<i32>> = Option::deserialize(d)?;
    if let Some(invalid_fds) = invalid_fds {
        // If the live-migration path is used properly, new FDs are passed as
        // SCM_RIGHTS message. So, we don't get them from the serialized JSON
        // anyway.
        debug!(
            "FDs in 'RestoredNetConfig' won't be deserialized as they are most likely invalid now. Deserializing them as -1."
        );
        Ok(Some(vec![-1; invalid_fds.len()]))
    } else {
        Ok(None)
    }
}

impl From<RestoredNetConfig> for config::RestoredNetConfig {
    fn from(value: RestoredNetConfig) -> Self {
        Self {
            id: value.id,
            num_fds: value.num_fds,
            fds: value.fds,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::api::types::{
        MemoryRestoreMode, ParseRestoreError, RestoreConfig, RestoredNetConfig,
    };

    #[test]
    fn test_restore_parsing() -> Result<(), ParseRestoreError> {
        assert_eq!(
            RestoreConfig::parse("source_url=/path/to/snapshot")?,
            RestoreConfig {
                source_url: PathBuf::from("/path/to/snapshot"),
                prefault: false,
                memory_restore_mode: MemoryRestoreMode::Copy,
                net_fds: None,
                resume: false,
            }
        );
        assert_eq!(
            RestoreConfig::parse(
                "source_url=/path/to/snapshot,prefault=off,net_fds=[net0@[3,4],net1@[5,6,7,8]]"
            )?,
            RestoreConfig {
                source_url: PathBuf::from("/path/to/snapshot"),
                prefault: false,
                memory_restore_mode: MemoryRestoreMode::Copy,
                net_fds: Some(vec![
                    RestoredNetConfig {
                        id: "net0".to_string(),
                        num_fds: 2,
                        fds: Some(vec![3, 4]),
                    },
                    RestoredNetConfig {
                        id: "net1".to_string(),
                        num_fds: 4,
                        fds: Some(vec![5, 6, 7, 8]),
                    }
                ]),
                resume: false,
            }
        );
        assert_eq!(
            RestoreConfig::parse("source_url=/path/to/snapshot,memory_restore_mode=ondemand")?,
            RestoreConfig {
                source_url: PathBuf::from("/path/to/snapshot"),
                prefault: false,
                memory_restore_mode: MemoryRestoreMode::OnDemand,
                net_fds: None,
                resume: false,
            }
        );
        assert_eq!(
            RestoreConfig::parse("source_url=/path/to/snapshot,resume=on")?,
            RestoreConfig {
                source_url: PathBuf::from("/path/to/snapshot"),
                prefault: false,
                memory_restore_mode: MemoryRestoreMode::Copy,
                net_fds: None,
                resume: true,
            }
        );
        // Parsing should fail as source_url is a required field
        RestoreConfig::parse("prefault=off").unwrap_err();
        RestoreConfig::parse("source_url=/path/to/snapshot,memory_restore_mode=bogus").unwrap_err();
        Ok(())
    }

    #[test]
    fn test_restore_config_serde() {
        assert_eq!(
            serde_json::from_str::<RestoreConfig>(r#"{"source_url":"/path/to/snapshot"}"#)
                .unwrap()
                .memory_restore_mode,
            MemoryRestoreMode::Copy
        );
        assert_eq!(
            serde_json::from_str::<RestoreConfig>(
                r#"{"source_url":"/path/to/snapshot","memory_restore_mode":"OnDemand"}"#
            )
            .unwrap()
            .memory_restore_mode,
            MemoryRestoreMode::OnDemand
        );
    }
}
