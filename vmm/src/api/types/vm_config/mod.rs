use std::path::PathBuf;

use option_parser::{OptionParser, Toggle};
use serde::{Deserialize, Serialize};

use crate::config::{Error, ValidationError, VmParams};
use crate::vm_config;

mod balloon;
mod console;
mod cpus;
mod devices;
mod disk;
mod fs_config;
mod generic_vhost_user;
mod memory;
mod net;
mod numa;
mod payload;
mod platform;
mod pmem;
mod rate_limiter_group;
mod rng;

pub use balloon::BalloonConfig;
pub use console::{ConsoleConfig, DebugConsoleConfig, SerialConfig};
pub use cpus::CpusConfig;
#[cfg(feature = "ivshmem")]
pub use devices::IvshmemConfig;
#[cfg(feature = "pvmemcontrol")]
pub use devices::PvmemcontrolConfig;
pub use devices::{
    DeviceConfig, LandlockConfig, PciSegmentConfig, RtcConfig, TpmConfig, UserDeviceConfig,
    VdpaConfig, VsockConfig,
};
pub use disk::DiskConfig;
pub use fs_config::FsConfig;
pub use generic_vhost_user::GenericVhostUserConfig;
pub use memory::MemoryConfig;
pub use net::NetConfig;
pub use numa::NumaConfig;
#[cfg(feature = "fw_cfg")]
pub use payload::FwCfgConfig;
pub use payload::PayloadConfig;
pub use platform::PlatformConfig;
pub use pmem::PmemConfig;
pub use rate_limiter_group::RateLimiterGroupConfig;
pub use rng::RngConfig;

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct PciDeviceCommonConfig {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "<&bool as std::ops::Not>::not")]
    pub iommu: bool,
    #[serde(default)]
    pub pci_segment: u16,
    #[serde(default)]
    pub pci_device_id: Option<u8>,
}

impl From<PciDeviceCommonConfig> for vm_config::PciDeviceCommonConfig {
    fn from(value: PciDeviceCommonConfig) -> Self {
        Self {
            id: value.id,
            iommu: value.iommu,
            pci_segment: value.pci_segment,
            pci_device_id: value.pci_device_id,
        }
    }
}

impl From<&vm_config::PciDeviceCommonConfig> for PciDeviceCommonConfig {
    fn from(value: &vm_config::PciDeviceCommonConfig) -> Self {
        Self {
            id: value.id.clone(),
            iommu: value.iommu,
            pci_segment: value.pci_segment,
            pci_device_id: value.pci_device_id,
        }
    }
}

impl PciDeviceCommonConfig {
    const OPTIONS: &[&str] = &["id", "pci_segment", "pci_device_id"];
    pub(crate) const OPTIONS_IOMMU: &[&str] = &["id", "iommu", "pci_segment", "pci_device_id"];

    pub fn parse(input: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add_all(Self::OPTIONS_IOMMU);
        parser
            .parse_subset(input)
            .map_err(Error::ParsePciDeviceCommonConfig)?;

        Ok(Self {
            id: parser.get("id"),
            iommu: parser
                .convert::<Toggle>("iommu")
                .map_err(Error::ParsePciDeviceCommonConfig)?
                .unwrap_or(Toggle(false))
                .0,
            pci_segment: parser
                .convert("pci_segment")
                .map_err(Error::ParsePciDeviceCommonConfig)?
                .unwrap_or_default(),
            pci_device_id: parser
                .convert::<u8>("pci_device_id")
                .map_err(Error::ParsePciDeviceCommonConfig)?,
        })
    }
}

#[serde_with::skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct VmConfig {
    #[serde(default)]
    pub cpus: CpusConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    pub payload: Option<PayloadConfig>,
    pub rate_limit_groups: Option<Box<[RateLimiterGroupConfig]>>,
    pub disks: Option<Vec<DiskConfig>>,
    pub net: Option<Vec<NetConfig>>,
    #[serde(default)]
    pub rng: RngConfig,
    pub balloon: Option<BalloonConfig>,
    pub generic_vhost_user: Option<Vec<GenericVhostUserConfig>>,
    pub fs: Option<Vec<FsConfig>>,
    pub pmem: Option<Vec<PmemConfig>>,
    #[serde(default)]
    pub serial: SerialConfig,
    #[serde(default)]
    pub console: ConsoleConfig,
    #[cfg(target_arch = "x86_64")]
    #[serde(default)]
    pub debug_console: DebugConsoleConfig,
    pub devices: Option<Vec<DeviceConfig>>,
    pub user_devices: Option<Vec<UserDeviceConfig>>,
    pub vdpa: Option<Vec<VdpaConfig>>,
    pub vsock: Option<VsockConfig>,
    #[cfg(feature = "pvmemcontrol")]
    #[serde(default)]
    pub pvmemcontrol: Option<PvmemcontrolConfig>,
    #[serde(default)]
    pub pvpanic: bool,
    #[serde(default)]
    pub iommu: bool,
    pub numa: Option<Box<[NumaConfig]>>,
    #[serde(default)]
    pub watchdog: bool,
    #[serde(default)]
    pub rtc: Option<RtcConfig>,
    #[cfg(feature = "guest_debug")]
    #[serde(default)]
    pub gdb: bool,
    pub pci_segments: Option<Box<[PciSegmentConfig]>>,
    pub platform: Option<PlatformConfig>,
    pub tpm: Option<TpmConfig>,
    #[serde(default)]
    pub landlock_enable: bool,
    pub landlock_rules: Option<Box<[LandlockConfig]>>,
    #[cfg(feature = "ivshmem")]
    pub ivshmem: Option<IvshmemConfig>,
}

impl TryFrom<VmConfig> for vm_config::VmConfig {
    type Error = ValidationError;

    fn try_from(value: VmConfig) -> Result<Self, Self::Error> {
        let mut config = Self {
            cpus: value.cpus.into(),
            memory: value.memory.into(),
            payload: value.payload.map(Into::into),
            rate_limit_groups: value.rate_limit_groups.map(|rate_limit_groups| {
                rate_limit_groups
                    .into_vec()
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            disks: value
                .disks
                .map(|disks| disks.into_iter().map(Into::into).collect()),
            net: value
                .net
                .map(|net| net.into_iter().map(Into::into).collect()),
            rng: value.rng.into(),
            balloon: value.balloon.map(Into::into),
            generic_vhost_user: value
                .generic_vhost_user
                .map(|devices| devices.into_iter().map(Into::into).collect()),
            fs: value.fs.map(|fs| fs.into_iter().map(Into::into).collect()),
            pmem: value
                .pmem
                .map(|pmem| pmem.into_iter().map(Into::into).collect()),
            serial: value.serial.into(),
            console: value.console.into(),
            #[cfg(target_arch = "x86_64")]
            debug_console: value.debug_console.into(),
            devices: value
                .devices
                .map(|devices| devices.into_iter().map(Into::into).collect()),
            user_devices: value
                .user_devices
                .map(|devices| devices.into_iter().map(Into::into).collect()),
            vdpa: value
                .vdpa
                .map(|devices| devices.into_iter().map(Into::into).collect()),
            vsock: value.vsock.map(Into::into),
            #[cfg(feature = "pvmemcontrol")]
            pvmemcontrol: value.pvmemcontrol.map(Into::into),
            pvpanic: value.pvpanic,
            iommu: value.iommu,
            numa: value.numa.map(|numa| {
                numa.into_vec()
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            watchdog: value.watchdog,
            rtc: value.rtc.map(Into::into),
            #[cfg(feature = "guest_debug")]
            gdb: value.gdb,
            pci_segments: value.pci_segments.map(|pci_segments| {
                pci_segments
                    .into_vec()
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            platform: value.platform.map(Into::into),
            tpm: value.tpm.map(Into::into),
            preserved_fds: None,
            landlock_enable: value.landlock_enable,
            landlock_rules: value.landlock_rules.map(|rules| {
                rules
                    .into_vec()
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            #[cfg(feature = "ivshmem")]
            ivshmem: value.ivshmem.map(Into::into),
        };

        config.validate()?;
        Ok(config)
    }
}

impl From<&vm_config::VmConfig> for VmConfig {
    fn from(value: &vm_config::VmConfig) -> Self {
        Self {
            cpus: (&value.cpus).into(),
            memory: (&value.memory).into(),
            payload: value.payload.as_ref().map(Into::into),
            rate_limit_groups: value.rate_limit_groups.as_ref().map(|rate_limit_groups| {
                rate_limit_groups
                    .iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            disks: value
                .disks
                .as_ref()
                .map(|disks| disks.iter().map(Into::into).collect()),
            net: value
                .net
                .as_ref()
                .map(|net| net.iter().map(Into::into).collect()),
            rng: (&value.rng).into(),
            balloon: value.balloon.as_ref().map(Into::into),
            generic_vhost_user: value
                .generic_vhost_user
                .as_ref()
                .map(|devices| devices.iter().map(Into::into).collect()),
            fs: value
                .fs
                .as_ref()
                .map(|fs| fs.iter().map(Into::into).collect()),
            pmem: value
                .pmem
                .as_ref()
                .map(|pmem| pmem.iter().map(Into::into).collect()),
            serial: (&value.serial).into(),
            console: (&value.console).into(),
            #[cfg(target_arch = "x86_64")]
            debug_console: (&value.debug_console).into(),
            devices: value
                .devices
                .as_ref()
                .map(|devices| devices.iter().map(Into::into).collect()),
            user_devices: value
                .user_devices
                .as_ref()
                .map(|devices| devices.iter().map(Into::into).collect()),
            vdpa: value
                .vdpa
                .as_ref()
                .map(|devices| devices.iter().map(Into::into).collect()),
            vsock: value.vsock.as_ref().map(Into::into),
            #[cfg(feature = "pvmemcontrol")]
            pvmemcontrol: value.pvmemcontrol.as_ref().map(Into::into),
            pvpanic: value.pvpanic,
            iommu: value.iommu,
            numa: value.numa.as_ref().map(|numa| {
                numa.iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            watchdog: value.watchdog,
            rtc: value.rtc.as_ref().map(Into::into),
            #[cfg(feature = "guest_debug")]
            gdb: value.gdb,
            pci_segments: value.pci_segments.as_ref().map(|pci_segments| {
                pci_segments
                    .iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            platform: value.platform.as_ref().map(Into::into),
            tpm: value.tpm.as_ref().map(Into::into),
            landlock_enable: value.landlock_enable,
            landlock_rules: value.landlock_rules.as_ref().map(|rules| {
                rules
                    .iter()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
            #[cfg(feature = "ivshmem")]
            ivshmem: value.ivshmem.as_ref().map(Into::into),
        }
    }
}

impl VmConfig {
    pub fn parse(vm_params: VmParams) -> Result<Self, Error> {
        let mut rate_limit_groups: Option<Box<[RateLimiterGroupConfig]>> = None;
        if let Some(rate_limit_group_list) = &vm_params.rate_limit_groups {
            let mut rate_limit_group_config_list = Vec::new();
            for item in rate_limit_group_list.iter() {
                let rate_limit_group_config = RateLimiterGroupConfig::parse(item)?;
                rate_limit_group_config_list.push(rate_limit_group_config);
            }
            rate_limit_groups = Some(rate_limit_group_config_list.into_boxed_slice());
        }

        let mut disks: Option<Vec<DiskConfig>> = None;
        if let Some(disk_list) = &vm_params.disks {
            let mut disk_config_list = Vec::new();
            for item in disk_list.iter() {
                let disk_config = DiskConfig::parse(item)?;
                disk_config_list.push(disk_config);
            }
            disks = Some(disk_config_list);
        }

        #[cfg(feature = "fw_cfg")]
        let fw_cfg_config = if let Some(fw_cfg_config_str) = vm_params.fw_cfg_config {
            let fw_cfg_config = FwCfgConfig::parse(fw_cfg_config_str)?;
            Some(fw_cfg_config)
        } else {
            None
        };

        let mut net: Option<Vec<NetConfig>> = None;
        if let Some(net_list) = &vm_params.net {
            let mut net_config_list = Vec::new();
            for item in net_list.iter() {
                let net_config = NetConfig::parse(item)?;
                net_config_list.push(net_config);
            }
            net = Some(net_config_list);
        }

        let rng = RngConfig::parse(vm_params.rng)?;

        let mut rtc: Option<RtcConfig> = None;
        if let Some(rtc_params) = &vm_params.rtc {
            rtc = Some(RtcConfig::parse(rtc_params)?);
        }

        let mut balloon: Option<BalloonConfig> = None;
        if let Some(balloon_params) = &vm_params.balloon {
            balloon = Some(BalloonConfig::parse(balloon_params)?);
        }

        #[cfg(feature = "pvmemcontrol")]
        let pvmemcontrol: Option<PvmemcontrolConfig> = vm_params
            .pvmemcontrol
            .then_some(PvmemcontrolConfig::default());

        let mut fs: Option<Vec<FsConfig>> = None;
        if let Some(fs_list) = &vm_params.fs {
            let mut fs_config_list = Vec::new();
            for item in fs_list.iter() {
                fs_config_list.push(FsConfig::parse(item)?);
            }
            fs = Some(fs_config_list);
        }

        let mut generic_vhost_user: Option<Vec<GenericVhostUserConfig>> = None;
        if let Some(generic_vhost_user_list) = &vm_params.generic_vhost_user {
            let mut generic_vhost_user_config_list = Vec::new();
            for item in generic_vhost_user_list.iter() {
                generic_vhost_user_config_list.push(GenericVhostUserConfig::parse(item)?);
            }
            generic_vhost_user = Some(generic_vhost_user_config_list);
        }

        let mut pmem: Option<Vec<PmemConfig>> = None;
        if let Some(pmem_list) = &vm_params.pmem {
            let mut pmem_config_list = Vec::new();
            for item in pmem_list.iter() {
                let pmem_config = PmemConfig::parse(item)?;
                pmem_config_list.push(pmem_config);
            }
            pmem = Some(pmem_config_list);
        }

        let console = ConsoleConfig::parse(vm_params.console)?;
        let serial = SerialConfig::parse(vm_params.serial)?;
        #[cfg(target_arch = "x86_64")]
        let debug_console = DebugConsoleConfig::parse(vm_params.debug_console)?;

        let mut devices: Option<Vec<DeviceConfig>> = None;
        if let Some(device_list) = &vm_params.devices {
            let mut device_config_list = Vec::new();
            for item in device_list.iter() {
                let device_config = DeviceConfig::parse(item)?;
                device_config_list.push(device_config);
            }
            devices = Some(device_config_list);
        }

        let mut user_devices: Option<Vec<UserDeviceConfig>> = None;
        if let Some(user_device_list) = &vm_params.user_devices {
            let mut user_device_config_list = Vec::new();
            for item in user_device_list.iter() {
                let user_device_config = UserDeviceConfig::parse(item)?;
                user_device_config_list.push(user_device_config);
            }
            user_devices = Some(user_device_config_list);
        }

        let mut vdpa: Option<Vec<VdpaConfig>> = None;
        if let Some(vdpa_list) = &vm_params.vdpa {
            let mut vdpa_config_list = Vec::new();
            for item in vdpa_list.iter() {
                let vdpa_config = VdpaConfig::parse(item)?;
                vdpa_config_list.push(vdpa_config);
            }
            vdpa = Some(vdpa_config_list);
        }

        let mut vsock: Option<VsockConfig> = None;
        if let Some(vs) = &vm_params.vsock {
            let vsock_config = VsockConfig::parse(vs)?;
            vsock = Some(vsock_config);
        }

        let mut pci_segments: Option<Box<[PciSegmentConfig]>> = None;
        if let Some(pci_segment_list) = &vm_params.pci_segments {
            let mut pci_segment_config_list = Vec::new();
            for item in pci_segment_list.iter() {
                let pci_segment_config = PciSegmentConfig::parse(item)?;
                pci_segment_config_list.push(pci_segment_config);
            }
            pci_segments = Some(pci_segment_config_list.into_boxed_slice());
        }

        let platform = vm_params.platform.map(PlatformConfig::parse).transpose()?;

        let mut numa: Option<Box<[NumaConfig]>> = None;
        if let Some(numa_list) = &vm_params.numa {
            let mut numa_config_list = Vec::new();
            for item in numa_list.iter() {
                let numa_config = NumaConfig::parse(item)?;
                numa_config_list.push(numa_config);
            }
            numa = Some(numa_config_list.into_boxed_slice());
        }

        #[cfg(not(feature = "igvm"))]
        let payload_present = vm_params.kernel.is_some() || vm_params.firmware.is_some();

        #[cfg(feature = "igvm")]
        let payload_present =
            vm_params.kernel.is_some() || vm_params.firmware.is_some() || vm_params.igvm.is_some();

        let payload = if payload_present {
            Some(PayloadConfig {
                kernel: vm_params.kernel.map(PathBuf::from),
                initramfs: vm_params.initramfs.map(PathBuf::from),
                cmdline: vm_params.cmdline.map(|s| s.to_string()),
                firmware: vm_params.firmware.map(PathBuf::from),
                #[cfg(feature = "igvm")]
                igvm: vm_params.igvm.map(PathBuf::from),
                #[cfg(feature = "sev_snp")]
                host_data: vm_params.host_data.map(|s| s.to_string()),
                #[cfg(feature = "fw_cfg")]
                fw_cfg_config,
            })
        } else {
            None
        };

        let mut tpm: Option<TpmConfig> = None;
        if let Some(tc) = vm_params.tpm {
            let tpm_conf = TpmConfig::parse(tc)?;
            tpm = Some(TpmConfig {
                socket: tpm_conf.socket,
            });
        }

        #[cfg(feature = "guest_debug")]
        let gdb = vm_params.gdb;

        let mut landlock_rules: Option<Box<[LandlockConfig]>> = None;
        if let Some(ll_rules) = vm_params.landlock_rules {
            landlock_rules = Some(
                ll_rules
                    .iter()
                    .map(|rule| LandlockConfig::parse(rule))
                    .collect::<Result<Vec<LandlockConfig>, Error>>()?
                    .into_boxed_slice(),
            );
        }

        #[cfg(feature = "ivshmem")]
        let mut ivshmem: Option<IvshmemConfig> = None;
        #[cfg(feature = "ivshmem")]
        if let Some(iv) = vm_params.ivshmem {
            let ivshmem_conf = IvshmemConfig::parse(iv)?;
            ivshmem = Some(ivshmem_conf);
        }

        let config = VmConfig {
            cpus: CpusConfig::parse(vm_params.cpus)?,
            memory: MemoryConfig::parse(vm_params.memory, vm_params.memory_zones)?,
            payload,
            rate_limit_groups,
            disks,
            net,
            rng,
            balloon,
            generic_vhost_user,
            fs,
            pmem,
            serial,
            console,
            #[cfg(target_arch = "x86_64")]
            debug_console,
            devices,
            user_devices,
            vdpa,
            vsock,
            #[cfg(feature = "pvmemcontrol")]
            pvmemcontrol,
            pvpanic: vm_params.pvpanic,
            iommu: false, // updated in VmConfig::validate()
            numa,
            watchdog: vm_params.watchdog,
            rtc,
            #[cfg(feature = "guest_debug")]
            gdb,
            pci_segments,
            platform,
            tpm,
            landlock_enable: vm_params.landlock_enable,
            landlock_rules,
            #[cfg(feature = "ivshmem")]
            ivshmem,
        };
        // TODO(ser)
        // config.validate().map_err(Error::Validation)?;
        Ok(config)
    }
}
