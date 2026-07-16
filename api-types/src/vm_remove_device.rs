use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, Default, Debug)]
pub struct VmRemoveDeviceData {
    pub id: String,
}
