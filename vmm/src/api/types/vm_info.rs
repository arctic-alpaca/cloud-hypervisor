use std::collections::HashMap;
use std::result;

use pci::PciBdf;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use vm_device::Resource;

use crate::api::types::VmConfig;
use crate::{device_tree, vm};

#[serde_with::skip_serializing_none]
#[derive(Clone, Deserialize, Serialize)]
pub struct VmInfoResponse {
    pub config: Box<VmConfig>,
    pub state: VmState,
    pub memory_actual_size: u64,
    pub device_tree: Option<DeviceTree>,
}

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

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct DeviceTree(HashMap<String, DeviceNode>);

impl From<device_tree::DeviceTree> for DeviceTree {
    fn from(value: device_tree::DeviceTree) -> Self {
        Self(
            value
                .iter()
                .map(|(key, value)| (key.clone(), value.clone().into()))
                .collect(),
        )
    }
}

#[serde_with::skip_serializing_none]
#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceNode {
    pub id: String,
    pub resources: Vec<Resource>,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub pci_bdf: Option<PciBdf>,
}

impl From<device_tree::DeviceNode> for DeviceNode {
    fn from(value: device_tree::DeviceNode) -> Self {
        Self {
            id: value.id,
            resources: value.resources,
            parent: value.parent,
            children: value.children,
            pci_bdf: value.pci_bdf,
        }
    }
}
