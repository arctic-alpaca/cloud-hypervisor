use serde::{Deserialize, Serialize};

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
