use std::ops::Deref;
use std::sync::{Arc, Mutex};

#[cfg(all(feature = "kvm", target_arch = "x86_64"))]
use hypervisor::arch::x86;
use serde::{Deserialize, Serialize};

use crate::api::types::VmConfig;
use crate::config::ValidationError;
use crate::memory_manager::MemoryManagerSnapshotData;

#[derive(Clone, Deserialize, Serialize)]
pub struct VmMigrationConfig {
    vm_config: VmConfig,
    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    common_cpuid: Vec<x86::CpuIdEntry>,
    memory_manager_data: MemoryManagerSnapshotData,
}

impl TryFrom<VmMigrationConfig> for super::VmMigrationConfig {
    type Error = ValidationError;

    fn try_from(value: VmMigrationConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            vm_config: Arc::new(Mutex::new(value.vm_config.try_into()?)),
            #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
            common_cpuid: value.common_cpuid,
            memory_manager_data: value.memory_manager_data,
        })
    }
}

impl From<&super::VmMigrationConfig> for VmMigrationConfig {
    fn from(value: &super::VmMigrationConfig) -> Self {
        Self {
            vm_config: value.vm_config.lock().unwrap().deref().into(),
            #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
            common_cpuid: value.common_cpuid.clone(),
            memory_manager_data: value.memory_manager_data.clone(),
        }
    }
}
