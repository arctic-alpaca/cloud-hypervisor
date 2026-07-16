use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, Default, Debug)]
pub struct VmResizeData {
    pub desired_vcpus: Option<u32>,
    pub desired_ram: Option<u64>,
    pub desired_balloon: Option<u64>,
}
