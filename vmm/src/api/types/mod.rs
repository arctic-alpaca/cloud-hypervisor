mod vm_coredump;
mod vm_device;
mod vm_resize;
mod vm_snapshot;
mod vmm_ping;

pub use vm_coredump::VmCoredumpData;
pub use vm_device::VmRemoveDeviceData;
pub use vm_resize::{VmResizeData, VmResizeDiskData, VmResizeZoneData};
pub use vm_snapshot::VmSnapshotConfig;
pub use vmm_ping::VmmPingResponse;
