use std::result;

use pci::PciBdf;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::vm;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum VmState {
    Created,
    Running,
    Shutdown,
    Paused,
    BreakPoint,
}

impl From<vm::VmState> for VmState {
    fn from(value: vm::VmState) -> Self {
        match value {
            vm::VmState::Created => Self::Created,
            vm::VmState::Running => Self::Running,
            vm::VmState::Shutdown => Self::Shutdown,
            vm::VmState::Paused => Self::Paused,
            vm::VmState::BreakPoint => Self::BreakPoint,
        }
    }
}

pub struct PciDeviceInfo {
    pub id: String,
    pub bdf: PciBdf,
}

impl Serialize for PciDeviceInfo {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bdf_str = self.bdf.to_string();

        // Serialize the structure.
        let mut state = serializer.serialize_struct("PciDeviceInfo", 2)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("bdf", &bdf_str)?;
        state.end()
    }
}

impl From<crate::PciDeviceInfo> for PciDeviceInfo {
    fn from(value: crate::PciDeviceInfo) -> Self {
        Self {
            id: value.id,
            bdf: value.bdf,
        }
    }
}
