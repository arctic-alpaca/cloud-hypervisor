// Copyright © 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashSet;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::{fs, result};

use arch::CpuProfile;
use block::ImageType;
pub use block::fcntl::LockGranularityChoice;
#[cfg(target_arch = "x86_64")]
use devices::debug_console;
use log::warn;
use net_util::MacAddr;
use thiserror::Error;
use virtio_devices::RateLimiterConfig;

use crate::Landlock;
use crate::landlock::LandlockError;

pub type LandlockResult<T> = result::Result<T, LandlockError>;

/// Trait to apply Landlock on VmConfig elements
pub(crate) trait ApplyLandlock {
    /// Apply Landlock rules to file paths
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuAffinity {
    pub vcpu: u32,
    pub host_cpus: Box<[usize]>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CpuFeatures {
    #[cfg(target_arch = "x86_64")]
    pub amx: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CoreScheduling {
    #[default]
    Vm, // All vCPUs have the same cookie so can share a core
    Vcpu, // Each vCPU has a unique cookie so can't share a core
    Off,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuTopology {
    pub threads_per_core: u16,
    pub cores_per_die: u16,
    pub dies_per_package: u16,
    pub packages: u16,
}

// When booting with PVH boot the maximum physical addressable size
// is a 46 bit address space even when the host supports with 5-level
// paging.
pub const DEFAULT_MAX_PHYS_BITS: u8 = 46;

pub fn default_cpuconfig_max_phys_bits() -> u8 {
    DEFAULT_MAX_PHYS_BITS
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpusConfig {
    pub boot_vcpus: u32,
    pub max_vcpus: u32,
    pub topology: Option<CpuTopology>,
    pub kvm_hyperv: bool,
    pub max_phys_bits: u8,
    pub affinity: Option<Box<[CpuAffinity]>>,
    pub features: CpuFeatures,
    pub nested: bool,
    pub core_scheduling: CoreScheduling,
    // Defaults to "Host" if no profile is given.
    pub profile: CpuProfile,
}

pub const DEFAULT_VCPUS: u32 = 1;

impl Default for CpusConfig {
    fn default() -> Self {
        CpusConfig {
            boot_vcpus: DEFAULT_VCPUS,
            max_vcpus: DEFAULT_VCPUS,
            topology: None,
            kvm_hyperv: false,
            max_phys_bits: DEFAULT_MAX_PHYS_BITS,
            affinity: None,
            features: CpuFeatures::default(),
            nested: true,
            core_scheduling: CoreScheduling::default(),
            profile: CpuProfile::default(),
        }
    }
}

pub const DEFAULT_NUM_PCI_SEGMENTS: u16 = 1;
pub fn default_platformconfig_num_pci_segments() -> u16 {
    DEFAULT_NUM_PCI_SEGMENTS
}

pub const DEFAULT_IOMMU_ADDRESS_WIDTH_BITS: u8 = 64;
pub fn default_platformconfig_iommu_address_width_bits() -> u8 {
    DEFAULT_IOMMU_ADDRESS_WIDTH_BITS
}

pub fn default_platformconfig_vfio_p2p_dma() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformConfig {
    pub num_pci_segments: u16,
    pub iommu_segments: Option<Box<[u16]>>,
    pub iommu_address_width_bits: u8,
    pub system_serial_number: Option<String>,
    pub system_uuid: Option<String>,
    pub oem_strings: Option<Box<[String]>>,
    pub system_manufacturer: Option<String>,
    pub system_product_name: Option<String>,
    pub system_version: Option<String>,
    pub system_family: Option<String>,
    pub system_sku_number: Option<String>,
    pub chassis_asset_tag: Option<String>,
    #[cfg(feature = "tdx")]
    pub tdx: bool,
    #[cfg(feature = "sev_snp")]
    pub sev_snp: bool,
    pub iommufd: bool,
    pub iommufd_fd: Option<i32>,
    pub vfio_p2p_dma: bool,
}

#[cfg(target_arch = "x86_64")]
impl PlatformConfig {
    /// Returns `None` if no SMBIOS-relevant platform fields are set, otherwise
    /// `Some` with a [`SmbiosConfig`] built from the populated fields.
    pub fn smbios_config(&self) -> Option<arch::x86_64::SmbiosConfig> {
        let has_system = [
            &self.system_serial_number,
            &self.system_uuid,
            &self.system_manufacturer,
            &self.system_product_name,
            &self.system_version,
            &self.system_family,
            &self.system_sku_number,
        ]
        .iter()
        .any(|v| v.is_some());

        let system = has_system.then_some(arch::x86_64::SmbiosSystem {
            manufacturer: self.system_manufacturer.clone(),
            product_name: self.system_product_name.clone(),
            version: self.system_version.clone(),
            serial_number: self.system_serial_number.clone(),
            uuid: self.system_uuid.clone(),
            sku_number: self.system_sku_number.clone(),
            family: self.system_family.clone(),
        });

        let chassis =
            self.chassis_asset_tag
                .clone()
                .map(|asset_tag| arch::x86_64::SmbiosChassisConfig {
                    asset_tag: Some(asset_tag),
                });

        let smbios = arch::x86_64::SmbiosConfig {
            system,
            chassis,
            oem_strings: self.oem_strings.clone().unwrap_or_default(),
        };

        (!smbios.is_empty()).then_some(smbios)
    }
}

pub const DEFAULT_PCI_SEGMENT_APERTURE_WEIGHT: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PciSegmentConfig {
    pub pci_segment: u16,
    pub mmio32_aperture_weight: u32,
    pub mmio64_aperture_weight: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryZoneConfig {
    pub id: String,
    pub size: u64,
    pub file: Option<PathBuf>,
    pub shared: bool,
    pub hugepages: bool,
    pub hugepage_size: Option<u64>,
    pub host_numa_node: Option<u32>,
    pub hotplug_size: Option<u64>,
    pub hotplugged_size: Option<u64>,
    pub prefault: bool,
    pub reserve: bool,
    pub mergeable: bool,
}

impl ApplyLandlock for MemoryZoneConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        if let Some(file) = &self.file {
            landlock.add_rule_with_access(file, "rw")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HotplugMethod {
    #[default]
    Acpi,
    VirtioMem,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryConfig {
    pub size: u64,
    pub mergeable: bool,
    pub hotplug_method: HotplugMethod,
    pub hotplug_size: Option<u64>,
    pub hotplugged_size: Option<u64>,
    pub shared: bool,
    pub hugepages: bool,
    pub hugepage_size: Option<u64>,
    pub prefault: bool,
    pub reserve: bool,
    pub zones: Option<Vec<MemoryZoneConfig>>,
    pub thp: bool,
}

pub const DEFAULT_MEMORY_MB: u64 = 512;

impl Default for MemoryConfig {
    fn default() -> Self {
        MemoryConfig {
            size: DEFAULT_MEMORY_MB << 20,
            mergeable: false,
            hotplug_method: HotplugMethod::Acpi,
            hotplug_size: None,
            hotplugged_size: None,
            shared: false,
            hugepages: false,
            hugepage_size: None,
            prefault: false,
            reserve: false,
            zones: None,
            thp: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum VhostMode {
    #[default]
    Client,
    Server,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimiterGroupConfig {
    pub id: String,
    pub rate_limiter_config: RateLimiterConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtQueueAffinity {
    pub queue_index: u16,
    pub host_cpus: Box<[usize]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PciDeviceCommonConfig {
    pub id: Option<String>,
    pub iommu: bool,
    pub pci_segment: u16,
    pub pci_device_id: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub path: Option<PathBuf>,
    pub readonly: bool,
    pub direct: bool,
    pub num_queues: usize,
    pub queue_size: u16,
    pub vhost_user: bool,
    pub vhost_socket: Option<String>,
    pub rate_limit_group: Option<String>,
    pub rate_limiter_config: Option<RateLimiterConfig>,
    pub disable_io_uring: bool,
    pub disable_aio: bool,
    pub serial: Option<String>,
    pub queue_affinity: Option<Box<[VirtQueueAffinity]>>,
    pub backing_files: bool,
    pub sparse: bool,
    pub image_type: ImageType,
    pub lock_granularity: LockGranularityChoice,
}

impl ApplyLandlock for DiskConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        if let Some(path) = &self.path {
            landlock.add_rule_with_access(path, "rw")?;
        }
        Ok(())
    }
}

pub const DEFAULT_DISK_NUM_QUEUES: usize = 1;

pub fn default_diskconfig_num_queues() -> usize {
    DEFAULT_DISK_NUM_QUEUES
}

pub const DEFAULT_DISK_QUEUE_SIZE: u16 = 128;

pub fn default_diskconfig_queue_size() -> u16 {
    DEFAULT_DISK_QUEUE_SIZE
}

pub fn default_diskconfig_sparse() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub tap: Option<String>,
    pub ip: Option<IpAddr>,
    pub mask: Option<IpAddr>,
    pub mac: MacAddr,
    pub host_mac: Option<MacAddr>,
    pub mtu: Option<u16>,
    pub num_queues: usize,
    pub queue_size: u16,
    pub vhost_user: bool,
    pub vhost_socket: Option<String>,
    pub vhost_mode: VhostMode,
    pub fds: Option<Vec<i32>>,
    pub rate_limiter_config: Option<RateLimiterConfig>,
    pub offload_tso: bool,
    pub offload_ufo: bool,
    pub offload_csum: bool,
}

pub fn default_netconfig_true() -> bool {
    true
}

pub fn default_netconfig_tap() -> Option<String> {
    None
}

pub fn default_netconfig_mac() -> MacAddr {
    MacAddr::local_random()
}

pub const DEFAULT_NET_NUM_QUEUES: usize = 2;

pub fn default_netconfig_num_queues() -> usize {
    DEFAULT_NET_NUM_QUEUES
}

pub const DEFAULT_NET_QUEUE_SIZE: u16 = 256;

pub fn default_netconfig_queue_size() -> u16 {
    DEFAULT_NET_QUEUE_SIZE
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RngConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub src: PathBuf,
}

impl RngConfig {
    pub const DEFAULT_RNG_SOURCE: &str = "/dev/urandom";
}

impl Default for RngConfig {
    fn default() -> Self {
        RngConfig {
            src: PathBuf::from(Self::DEFAULT_RNG_SOURCE),
            pci_common: PciDeviceCommonConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RtcConfig {
    pub pci_common: PciDeviceCommonConfig,
}

impl ApplyLandlock for RngConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        // Rng Path only need read access
        landlock.add_rule_with_access(&self.src, "r")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BalloonConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub size: u64,
    /// Option to deflate the balloon in case the guest is out of memory.
    pub deflate_on_oom: bool,
    /// Option to enable free page reporting from the guest.
    pub free_page_reporting: bool,
}

#[cfg(feature = "pvmemcontrol")]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PvmemcontrolConfig {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub tag: String,
    pub socket: PathBuf,
    pub num_queues: usize,
    pub queue_size: u16,
}

pub fn default_fsconfig_num_queues() -> usize {
    1
}

pub fn default_fsconfig_queue_size() -> u16 {
    1024
}

impl ApplyLandlock for FsConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        landlock.add_rule_with_access(&self.socket, "rw")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericVhostUserConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub socket: PathBuf,
    pub queue_sizes: Vec<u16>,
    pub device_type: u32,
}

impl ApplyLandlock for GenericVhostUserConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        landlock.add_rule_with_access(&self.socket, "rw")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PmemConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub file: PathBuf,
    pub size: Option<u64>,
    pub discard_writes: bool,
}

impl ApplyLandlock for PmemConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        let access = if self.discard_writes { "r" } else { "rw" };
        landlock.add_rule_with_access(&self.file, access)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsoleOutputMode {
    Off,
    Pty,
    Tty,
    File,
    Socket,
    Null,
}

/// Common configuration for plain console configs.
///
/// Independent of PCI or legacy devices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonConsoleConfig {
    pub file: Option<PathBuf>,
    pub mode: ConsoleOutputMode,
    pub socket: Option<PathBuf>,
}

impl ApplyLandlock for CommonConsoleConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        if self.mode == ConsoleOutputMode::Pty {
            landlock.add_rule_with_access(Path::new("/dev/pts"), "rw")?;
            landlock.add_rule_with_access(Path::new("/dev/ptmx"), "rw")?;
        }
        if let Some(file) = &self.file {
            landlock.add_rule_with_access(file, "rw")?;
        }
        if let Some(socket) = &self.socket {
            landlock.add_rule_with_access(socket, "rw")?;
        }
        Ok(())
    }
}

/// Configuration for a legacy serial console device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialConfig {
    pub common: CommonConsoleConfig,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            common: CommonConsoleConfig {
                file: None,
                mode: ConsoleOutputMode::Null,
                socket: None,
            },
        }
    }
}

impl ApplyLandlock for SerialConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        self.common.apply_landlock(landlock)
    }
}

/// Configuration for a virtio-console device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleConfig {
    pub common: CommonConsoleConfig,
    pub pci_common: PciDeviceCommonConfig,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self {
            common: CommonConsoleConfig {
                file: None,
                mode: ConsoleOutputMode::Tty,
                socket: None,
            },
            pci_common: PciDeviceCommonConfig::default(),
        }
    }
}

impl ApplyLandlock for ConsoleConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        self.common.apply_landlock(landlock)
    }
}

#[cfg(target_arch = "x86_64")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugConsoleConfig {
    pub file: Option<PathBuf>,
    pub mode: ConsoleOutputMode,
    /// Optionally dedicated I/O-port, if the default port should not be used.
    pub iobase: Option<u16>,
}

#[cfg(target_arch = "x86_64")]
impl Default for DebugConsoleConfig {
    fn default() -> Self {
        Self {
            file: None,
            mode: ConsoleOutputMode::Off,
            iobase: Some(debug_console::DEFAULT_PORT as u16),
        }
    }
}
#[cfg(target_arch = "x86_64")]
impl ApplyLandlock for DebugConsoleConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        if self.mode == ConsoleOutputMode::Pty {
            landlock.add_rule_with_access(Path::new("/dev/pts"), "rw")?;
            landlock.add_rule_with_access(Path::new("/dev/ptmx"), "rw")?;
        }
        if let Some(file) = &self.file {
            landlock.add_rule_with_access(file, "rw")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub path: Option<PathBuf>,
    pub fd: Option<i32>,
    pub x_nv_gpudirect_clique: Option<u8>,
    pub x_exclude_mmap_bars: Vec<u64>,
}

impl ApplyLandlock for DeviceConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        // When the device is supplied via an externally-opened FD, there is no
        // path to grant access to: the file is already open. Skip the rule.
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let device_path = fs::read_link(path).map_err(LandlockError::OpenPath)?;
        let iommu_group = device_path.file_name();
        let iommu_group_str = iommu_group
            .ok_or(LandlockError::InvalidPath)?
            .to_str()
            .ok_or(LandlockError::InvalidPath)?;

        let mut vfio_group_path = PathBuf::from("/dev/vfio");
        vfio_group_path.push(iommu_group_str);
        landlock.add_rule_with_access(&vfio_group_path, "rw")?;

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserDeviceConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub socket: PathBuf,
}

impl ApplyLandlock for UserDeviceConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        landlock.add_rule_with_access(&self.socket, "rw")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VdpaConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub path: PathBuf,
    pub num_queues: usize,
}

impl ApplyLandlock for VdpaConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        landlock.add_rule_with_access(&self.path, "rw")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VsockConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub cid: u32,
    pub socket: PathBuf,
}

impl ApplyLandlock for VsockConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        if let Some(parent) = self.socket.parent() {
            landlock.add_rule_with_access(parent, "w")?;
        }

        landlock.add_rule_with_access(&self.socket, "rw")?;

        Ok(())
    }
}

#[cfg(feature = "ivshmem")]
pub const DEFAULT_IVSHMEM_SIZE: usize = 128;

#[cfg(feature = "ivshmem")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IvshmemConfig {
    pub pci_common: PciDeviceCommonConfig,
    pub path: PathBuf,
    pub size: usize,
}

#[cfg(feature = "ivshmem")]
impl Default for IvshmemConfig {
    fn default() -> Self {
        Self {
            pci_common: PciDeviceCommonConfig::default(),
            path: PathBuf::new(),
            size: DEFAULT_IVSHMEM_SIZE << 20,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumaDistance {
    pub destination: u32,
    pub distance: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumaConfig {
    pub guest_numa_id: u32,
    pub cpus: Option<Box<[u32]>>,
    pub distances: Option<Box<[NumaDistance]>>,
    pub memory_zones: Option<Box<[String]>>,
    pub pci_segments: Option<Box<[u16]>>,
    pub device_id: Option<String>,
}

/// Errors describing a misconfigured payload, i.e., a configuration that
/// can't be booted by Cloud Hypervisor.
///
/// This typically is the case for invalid combinations of cmdline, kernel,
/// firmware, and initrd.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PayloadConfigError {
    /// Specifying a kernel is not supported when a firmware is provided.
    #[error("Specifying a kernel is not supported when a firmware is provided")]
    FirmwarePlusOtherPayloads,
    /// No bootitem provided: neither firmware nor kernel.
    #[error("No bootitem provided: neither firmware nor kernel")]
    MissingBootitem,
    #[cfg(feature = "igvm")]
    /// Specifying a kernel or firmware is not supported when an igvm is provided.
    #[error("Specifying a kernel or firmware is not supported when an igvm is provided")]
    IgvmPlusOtherPayloads,
    #[cfg(feature = "fw_cfg")]
    /// FwCfg missing kernel
    #[error("Error --fw-cfg-config: missing --kernel")]
    FwCfgMissingKernel,
    #[cfg(feature = "fw_cfg")]
    /// FwCfg missing cmdline
    #[error("Error --fw-cfg-config: missing --cmdline")]
    FwCfgMissingCmdline,
    #[cfg(feature = "fw_cfg")]
    /// FwCfg missing initramfs
    #[error("Error --fw-cfg-config: missing --initramfs")]
    FwCfgMissingInitramfs,
    #[cfg(feature = "fw_cfg")]
    /// Invalid fw_cfg item content
    #[error(
        "Error --fw-cfg-config: invalid item '{0}' (exactly one of 'file' or 'string' is required)"
    )]
    FwCfgInvalidItem(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadConfig {
    pub firmware: Option<PathBuf>,
    pub kernel: Option<PathBuf>,
    pub cmdline: Option<String>,
    pub initramfs: Option<PathBuf>,
    #[cfg(feature = "igvm")]
    pub igvm: Option<PathBuf>,
    #[cfg(feature = "sev_snp")]
    pub host_data: Option<String>,
    #[cfg(feature = "fw_cfg")]
    pub fw_cfg_config: Option<FwCfgConfig>,
}

#[cfg(feature = "fw_cfg")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FwCfgConfig {
    pub e820: bool,
    pub kernel: bool,
    pub cmdline: bool,
    pub initramfs: bool,
    pub acpi_tables: bool,
    pub items: Option<FwCfgItemList>,
}

#[cfg(feature = "fw_cfg")]
impl Default for FwCfgConfig {
    fn default() -> Self {
        FwCfgConfig {
            e820: true,
            kernel: true,
            cmdline: true,
            initramfs: true,
            acpi_tables: true,
            items: None,
        }
    }
}

#[cfg(feature = "fw_cfg")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FwCfgItemList {
    pub item_list: Vec<FwCfgItem>,
}

#[cfg(feature = "fw_cfg")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FwCfgItem {
    pub name: String,
    pub file: Option<PathBuf>,
    pub string: Option<String>,
}

impl PayloadConfig {
    /// Validates the payload config.
    ///
    /// Succeeds if Cloud Hypervisor will be able to boot the configuration.
    /// Further, warns for some odd configurations.
    pub fn validate(&mut self) -> Result<(), PayloadConfigError> {
        #[cfg(feature = "igvm")]
        {
            if self.igvm.is_some() {
                if self.firmware.is_some() {
                    return Err(PayloadConfigError::IgvmPlusOtherPayloads);
                }
                return Ok(());
            }
        }
        match (&self.firmware, &self.kernel) {
            (Some(_firmware), Some(_kernel)) => Err(PayloadConfigError::FirmwarePlusOtherPayloads),
            (Some(_firmware), None) => {
                if self.cmdline.is_some() {
                    warn!("Ignoring cmdline parameter as firmware is provided as the payload");
                    self.cmdline = None;
                }
                if self.initramfs.is_some() {
                    warn!("Ignoring initramfs parameter as firmware is provided as the payload");
                    self.initramfs = None;
                }
                Ok(())
            }
            (None, Some(_kernel)) => Ok(()),
            (None, None) => Err(PayloadConfigError::MissingBootitem),
        }?;

        #[cfg(feature = "fw_cfg")]
        if let Some(fw_cfg_config) = &self.fw_cfg_config {
            fw_cfg_config.validate(self)?;
        }

        Ok(())
    }
}

impl ApplyLandlock for PayloadConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        // Payload only needs read access
        if let Some(firmware) = &self.firmware {
            landlock.add_rule_with_access(firmware, "r")?;
        }

        if let Some(kernel) = &self.kernel {
            landlock.add_rule_with_access(kernel, "r")?;
        }

        if let Some(initramfs) = &self.initramfs {
            landlock.add_rule_with_access(initramfs, "r")?;
        }

        #[cfg(feature = "igvm")]
        if let Some(igvm) = &self.igvm {
            landlock.add_rule_with_access(igvm, "r")?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TpmConfig {
    pub socket: PathBuf,
}

impl ApplyLandlock for TpmConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        landlock.add_rule_with_access(&self.socket, "rw")?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandlockConfig {
    pub path: PathBuf,
    pub access: String,
}

impl ApplyLandlock for LandlockConfig {
    fn apply_landlock(&self, landlock: &mut Landlock) -> LandlockResult<()> {
        landlock.add_rule_with_access(&self.path, self.access.clone().as_str())?;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct VmConfig {
    pub cpus: CpusConfig,
    pub memory: MemoryConfig,
    pub payload: Option<PayloadConfig>,
    pub rate_limit_groups: Option<Box<[RateLimiterGroupConfig]>>,
    pub disks: Option<Vec<DiskConfig>>,
    pub net: Option<Vec<NetConfig>>,
    pub rng: RngConfig,
    pub balloon: Option<BalloonConfig>,
    pub generic_vhost_user: Option<Vec<GenericVhostUserConfig>>,
    pub fs: Option<Vec<FsConfig>>,
    pub pmem: Option<Vec<PmemConfig>>,
    pub serial: SerialConfig,
    pub console: ConsoleConfig,
    #[cfg(target_arch = "x86_64")]
    pub debug_console: DebugConsoleConfig,
    pub devices: Option<Vec<DeviceConfig>>,
    pub user_devices: Option<Vec<UserDeviceConfig>>,
    pub vdpa: Option<Vec<VdpaConfig>>,
    pub vsock: Option<VsockConfig>,
    #[cfg(feature = "pvmemcontrol")]
    pub pvmemcontrol: Option<PvmemcontrolConfig>,
    pub pvpanic: bool,
    pub iommu: bool,
    pub numa: Option<Box<[NumaConfig]>>,
    pub watchdog: bool,
    pub rtc: Option<RtcConfig>,
    #[cfg(feature = "guest_debug")]
    pub gdb: bool,
    pub pci_segments: Option<Box<[PciSegmentConfig]>>,
    pub platform: Option<PlatformConfig>,
    pub tpm: Option<TpmConfig>,
    // Preserved FDs are the ones that share the same life-time as its holding
    // VmConfig instance, such as FDs for creating TAP devices.
    // Preserved FDs will stay open as long as the holding VmConfig instance is
    // valid, and will be closed when the holding VmConfig instance is destroyed.
    //
    // This is populated as devices are added at runtime. Removing them again
    // causes the FDs to be closed early. This allows management software to
    // gracefully clean up resources (e.g., libvirt closes tap devices).
    pub preserved_fds: Option<HashSet<i32>>,
    pub landlock_enable: bool,
    pub landlock_rules: Option<Box<[LandlockConfig]>>,
    #[cfg(feature = "ivshmem")]
    pub ivshmem: Option<IvshmemConfig>,
}

impl VmConfig {
    pub(crate) fn apply_landlock(&self) -> LandlockResult<()> {
        let mut landlock = Landlock::new()?;

        #[cfg(target_arch = "aarch64")]
        {
            landlock.add_rule_with_access(Path::new("/sys/devices/system/cpu/cpu0/cache"), "r")?;
        }

        if let Some(mem_zones) = &self.memory.zones {
            for zone in mem_zones.iter() {
                zone.apply_landlock(&mut landlock)?;
            }
        }

        let disks = &self.disks;
        if let Some(disks) = disks {
            for disk in disks.iter() {
                disk.apply_landlock(&mut landlock)?;
            }
        }

        self.rng.apply_landlock(&mut landlock)?;

        if let Some(fs_configs) = &self.fs {
            for fs_config in fs_configs.iter() {
                fs_config.apply_landlock(&mut landlock)?;
            }
        }

        if let Some(generic_vhost_user_configs) = &self.generic_vhost_user {
            for generic_vhost_user_config in generic_vhost_user_configs.iter() {
                generic_vhost_user_config.apply_landlock(&mut landlock)?;
            }
        }

        if let Some(pmem_configs) = &self.pmem {
            for pmem_config in pmem_configs.iter() {
                pmem_config.apply_landlock(&mut landlock)?;
            }
        }

        self.console.apply_landlock(&mut landlock)?;
        self.serial.apply_landlock(&mut landlock)?;

        #[cfg(target_arch = "x86_64")]
        {
            self.debug_console.apply_landlock(&mut landlock)?;
        }

        if let Some(devices) = &self.devices {
            landlock.add_rule_with_access(Path::new("/dev/vfio/vfio"), "rw")?;

            for device in devices.iter() {
                device.apply_landlock(&mut landlock)?;
            }
        }

        if let Some(user_devices) = &self.user_devices {
            for user_devices in user_devices.iter() {
                user_devices.apply_landlock(&mut landlock)?;
            }
        }

        if let Some(vdpa_configs) = &self.vdpa {
            for vdpa_config in vdpa_configs.iter() {
                vdpa_config.apply_landlock(&mut landlock)?;
            }
        }

        if let Some(vsock_config) = &self.vsock {
            vsock_config.apply_landlock(&mut landlock)?;
        }

        if let Some(payload) = &self.payload {
            payload.apply_landlock(&mut landlock)?;
        }

        #[cfg(feature = "sev_snp")]
        if self.platform.as_ref().is_some_and(|p| p.sev_snp) {
            landlock.add_rule_with_access(Path::new("/dev/sev"), "rw")?;
        }

        if let Some(tpm_config) = &self.tpm {
            tpm_config.apply_landlock(&mut landlock)?;
        }

        if self.net.is_some() {
            landlock.add_rule_with_access(Path::new("/dev/net/tun"), "rw")?;
        }

        if let Some(landlock_rules) = &self.landlock_rules {
            for landlock_rule in landlock_rules.iter() {
                landlock_rule.apply_landlock(&mut landlock)?;
            }
        }

        landlock.restrict_self()?;

        Ok(())
    }

    #[cfg(all(feature = "kvm", target_arch = "x86_64"))]
    pub(crate) fn max_apic_id(&self) -> u32 {
        if let Some(topology) = &self.cpus.topology {
            arch::x86_64::get_max_x2apic_id((
                topology.threads_per_core,
                topology.cores_per_die,
                topology.dies_per_package,
                topology.packages,
            ))
        } else {
            self.cpus.max_vcpus
        }
    }
}
