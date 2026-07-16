use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize, Default, Debug)]
pub struct VmCoredumpData {
    /// The coredump destination file
    pub destination_url: String,
}
