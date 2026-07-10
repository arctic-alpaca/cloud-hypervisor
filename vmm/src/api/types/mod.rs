mod vm_device;
mod vm_resize;
mod vm_snapshot;

pub use vm_device::VmRemoveDeviceData;
pub use vm_resize::{VmResizeData, VmResizeDiskData, VmResizeZoneData};
pub use vm_snapshot::VmSnapshotConfig;
