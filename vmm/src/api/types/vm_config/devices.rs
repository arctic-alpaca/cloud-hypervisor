use std::path::PathBuf;

use log::debug;
#[cfg(feature = "ivshmem")]
use option_parser::ByteSized;
use option_parser::{IntegerList, OptionParser, OptionParserError};
use serde::{Deserialize, Serialize};

use crate::api::types::PciDeviceCommonConfig;
use crate::config::Error;
use crate::vm_config;
#[cfg(feature = "ivshmem")]
use crate::vm_config::DEFAULT_IVSHMEM_SIZE;
use crate::vm_config::DEFAULT_PCI_SEGMENT_APERTURE_WEIGHT;

#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct DeviceConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    #[serde(default)]
    pub path: Option<PathBuf>,
    // FDs are not serialized and any deserialized value is invalid; see NetConfig::fds.
    #[serde(default, deserialize_with = "deserialize_deviceconfig_fd")]
    pub fd: Option<i32>,
    #[serde(default)]
    pub x_nv_gpudirect_clique: Option<u8>,
    #[serde(default)]
    pub x_exclude_mmap_bars: Vec<u64>,
}

impl From<DeviceConfig> for vm_config::DeviceConfig {
    fn from(value: DeviceConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            path: value.path,
            fd: value.fd,
            x_nv_gpudirect_clique: value.x_nv_gpudirect_clique,
            x_exclude_mmap_bars: value.x_exclude_mmap_bars,
        }
    }
}

impl From<&vm_config::DeviceConfig> for DeviceConfig {
    fn from(value: &vm_config::DeviceConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            path: value.path.clone(),
            fd: value.fd,
            x_nv_gpudirect_clique: value.x_nv_gpudirect_clique,
            x_exclude_mmap_bars: value.x_exclude_mmap_bars.clone(),
        }
    }
}

impl DeviceConfig {
    pub const SYNTAX: &'static str = "Direct device assignment parameters \
    \"path=<device_path>,fd=<vfio_cdev_fd>,iommu=on|off,id=<device_id>,\
    pci_segment=<segment_id>,pci_device_id=<pci_slot>,\
    x_nv_gpudirect_clique=<clique_id>,\
    x_exclude_mmap_bars=[<bar>...]\"";

    pub fn parse(device: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("path")
            .add("fd")
            .add_all(PciDeviceCommonConfig::OPTIONS_IOMMU)
            .add("x_nv_gpudirect_clique")
            .add("x_exclude_mmap_bars");
        parser.parse(device).map_err(Error::ParseDevice)?;

        let pci_common = PciDeviceCommonConfig::parse(device)?;
        let path = parser.get("path").map(PathBuf::from);
        let fd = parser.convert::<i32>("fd").map_err(Error::ParseDevice)?;
        let x_nv_gpudirect_clique = parser
            .convert::<u8>("x_nv_gpudirect_clique")
            .map_err(Error::ParseDevice)?;
        let x_exclude_mmap_bars = parser
            .convert::<IntegerList>("x_exclude_mmap_bars")
            .map_err(Error::ParseDevice)?
            .map(|bars| bars.0)
            .unwrap_or_default();
        Ok(DeviceConfig {
            pci_common,
            path,
            fd,
            x_nv_gpudirect_clique,
            x_exclude_mmap_bars,
        })
    }
}

fn deserialize_deviceconfig_fd<'de, D>(d: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let invalid_fd: Option<i32> = Option::deserialize(d)?;
    if invalid_fd.is_some() {
        debug!(
            "FD in 'DeviceConfig' won't be deserialized as it is most likely invalid now. Deserializing it as -1."
        );
        Ok(Some(-1))
    } else {
        Ok(None)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UserDeviceConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub socket: PathBuf,
}

impl From<UserDeviceConfig> for vm_config::UserDeviceConfig {
    fn from(value: UserDeviceConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            socket: value.socket,
        }
    }
}

impl From<&vm_config::UserDeviceConfig> for UserDeviceConfig {
    fn from(value: &vm_config::UserDeviceConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            socket: value.socket.clone(),
        }
    }
}

impl UserDeviceConfig {
    pub const SYNTAX: &'static str = "Userspace device socket=<socket_path>,id=<device_id>,\
        pci_segment=<segment_id>,pci_device_id=<pci_slot>\"";

    pub fn parse(user_device: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add("socket").add_all(PciDeviceCommonConfig::OPTIONS);
        parser.parse(user_device).map_err(Error::ParseUserDevice)?;

        let pci_common = PciDeviceCommonConfig::parse(user_device)?;
        let socket = parser
            .get("socket")
            .map(PathBuf::from)
            .ok_or(Error::ParseUserDeviceSocketMissing)?;

        Ok(UserDeviceConfig { pci_common, socket })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct VdpaConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub path: PathBuf,
    #[serde(default = "default_vdpaconfig_num_queues")]
    pub num_queues: usize,
}

impl From<VdpaConfig> for vm_config::VdpaConfig {
    fn from(value: VdpaConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            path: value.path,
            num_queues: value.num_queues,
        }
    }
}

impl From<&vm_config::VdpaConfig> for VdpaConfig {
    fn from(value: &vm_config::VdpaConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            path: value.path.clone(),
            num_queues: value.num_queues,
        }
    }
}

impl VdpaConfig {
    pub const SYNTAX: &'static str = "vDPA device \
        \"path=<device_path>,num_queues=<number_of_queues>,iommu=on|off,\
        id=<device_id>,pci_segment=<segment_id>,pci_device_id=<pci_slot>\"";

    pub fn parse(vdpa: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("path")
            .add("num_queues")
            .add_all(PciDeviceCommonConfig::OPTIONS_IOMMU);
        parser.parse(vdpa).map_err(Error::ParseVdpa)?;

        let pci_common = PciDeviceCommonConfig::parse(vdpa)?;
        let path = parser
            .get("path")
            .map(PathBuf::from)
            .ok_or(Error::ParseVdpaPathMissing)?;
        let num_queues = parser
            .convert("num_queues")
            .map_err(Error::ParseVdpa)?
            .unwrap_or_else(default_vdpaconfig_num_queues);

        Ok(VdpaConfig {
            pci_common,
            path,
            num_queues,
        })
    }
}

pub fn default_vdpaconfig_num_queues() -> usize {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct VsockConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub cid: u32,
    pub socket: PathBuf,
}

impl From<VsockConfig> for vm_config::VsockConfig {
    fn from(value: VsockConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            cid: value.cid,
            socket: value.socket,
        }
    }
}

impl From<&vm_config::VsockConfig> for VsockConfig {
    fn from(value: &vm_config::VsockConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            cid: value.cid,
            socket: value.socket.clone(),
        }
    }
}

impl VsockConfig {
    pub const SYNTAX: &'static str = "Virtio VSOCK parameters \
        \"cid=<context_id>,socket=<socket_path>,iommu=on|off,id=<device_id>,\
        pci_segment=<segment_id>,pci_device_id=<pci_slot>\"";

    pub fn parse(vsock: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("socket")
            .add("cid")
            .add_all(PciDeviceCommonConfig::OPTIONS_IOMMU);
        parser.parse(vsock).map_err(Error::ParseVsock)?;

        let pci_common = PciDeviceCommonConfig::parse(vsock)?;
        let socket = parser
            .get("socket")
            .map(PathBuf::from)
            .ok_or(Error::ParseVsockSockMissing)?;
        let cid = parser
            .convert("cid")
            .map_err(Error::ParseVsock)?
            .ok_or(Error::ParseVsockCidMissing)?;

        Ok(VsockConfig {
            pci_common,
            cid,
            socket,
        })
    }
}

#[cfg(feature = "pvmemcontrol")]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct PvmemcontrolConfig {}

#[cfg(feature = "pvmemcontrol")]
impl From<PvmemcontrolConfig> for vm_config::PvmemcontrolConfig {
    fn from(_value: PvmemcontrolConfig) -> Self {
        Self {}
    }
}

#[cfg(feature = "pvmemcontrol")]
impl From<&vm_config::PvmemcontrolConfig> for PvmemcontrolConfig {
    fn from(_value: &vm_config::PvmemcontrolConfig) -> Self {
        Self {}
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RtcConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
}

impl From<RtcConfig> for vm_config::RtcConfig {
    fn from(value: RtcConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
        }
    }
}

impl From<&vm_config::RtcConfig> for RtcConfig {
    fn from(value: &vm_config::RtcConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
        }
    }
}

impl RtcConfig {
    pub const SYNTAX: &'static str = "Virtio RTC parameters \"\
        iommu=on|off,id=<device_id>,\
        pci_segment=<segment_id>,pci_device_id=<pci_slot>\". \
        Passing --rtc with no arguments enables the device with default \
        settings.";

    pub fn parse(rtc: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add_all(PciDeviceCommonConfig::OPTIONS_IOMMU);
        parser.parse(rtc).map_err(Error::ParseRtc)?;

        let pci_common = PciDeviceCommonConfig::parse(rtc)?;

        Ok(RtcConfig { pci_common })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct PciSegmentConfig {
    #[serde(default)]
    pub pci_segment: u16,
    #[serde(default = "default_pci_segment_aperture_weight")]
    pub mmio32_aperture_weight: u32,
    #[serde(default = "default_pci_segment_aperture_weight")]
    pub mmio64_aperture_weight: u32,
}

impl From<PciSegmentConfig> for vm_config::PciSegmentConfig {
    fn from(value: PciSegmentConfig) -> Self {
        Self {
            pci_segment: value.pci_segment,
            mmio32_aperture_weight: value.mmio32_aperture_weight,
            mmio64_aperture_weight: value.mmio64_aperture_weight,
        }
    }
}

impl From<&vm_config::PciSegmentConfig> for PciSegmentConfig {
    fn from(value: &vm_config::PciSegmentConfig) -> Self {
        Self {
            pci_segment: value.pci_segment,
            mmio32_aperture_weight: value.mmio32_aperture_weight,
            mmio64_aperture_weight: value.mmio64_aperture_weight,
        }
    }
}

impl PciSegmentConfig {
    pub const SYNTAX: &'static str = "PCI Segment parameters \
         \"pci_segment=<segment_id>,mmio32_aperture_weight=<scale>,mmio64_aperture_weight=<scale>\"";

    pub fn parse(disk: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser
            .add("mmio32_aperture_weight")
            .add("mmio64_aperture_weight")
            .add("pci_segment");
        parser.parse(disk).map_err(Error::ParsePciSegment)?;

        let pci_segment = parser
            .convert("pci_segment")
            .map_err(Error::ParsePciSegment)?
            .unwrap_or_default();
        let mmio32_aperture_weight = parser
            .convert("mmio32_aperture_weight")
            .map_err(Error::ParsePciSegment)?
            .unwrap_or(DEFAULT_PCI_SEGMENT_APERTURE_WEIGHT);
        let mmio64_aperture_weight = parser
            .convert("mmio64_aperture_weight")
            .map_err(Error::ParsePciSegment)?
            .unwrap_or(DEFAULT_PCI_SEGMENT_APERTURE_WEIGHT);

        Ok(PciSegmentConfig {
            pci_segment,
            mmio32_aperture_weight,
            mmio64_aperture_weight,
        })
    }
}

fn default_pci_segment_aperture_weight() -> u32 {
    DEFAULT_PCI_SEGMENT_APERTURE_WEIGHT
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TpmConfig {
    pub socket: PathBuf,
}

impl From<TpmConfig> for vm_config::TpmConfig {
    fn from(value: TpmConfig) -> Self {
        Self {
            socket: value.socket,
        }
    }
}

impl From<&vm_config::TpmConfig> for TpmConfig {
    fn from(value: &vm_config::TpmConfig) -> Self {
        Self {
            socket: value.socket.clone(),
        }
    }
}

impl TpmConfig {
    pub const SYNTAX: &'static str = "TPM device \
        \"(UNIX Domain Socket from swtpm) socket=</path/to/a/socket>\"";

    pub fn parse(tpm: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add("socket");
        parser.parse(tpm).map_err(Error::ParseTpm)?;
        let socket = parser
            .get("socket")
            .map(PathBuf::from)
            .ok_or(Error::ParseTpmPathMissing)?;
        Ok(TpmConfig { socket })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LandlockConfig {
    pub path: PathBuf,
    pub access: String,
}

impl From<LandlockConfig> for vm_config::LandlockConfig {
    fn from(value: LandlockConfig) -> Self {
        Self {
            path: value.path,
            access: value.access,
        }
    }
}

impl From<&vm_config::LandlockConfig> for LandlockConfig {
    fn from(value: &vm_config::LandlockConfig) -> Self {
        Self {
            path: value.path.clone(),
            access: value.access.clone(),
        }
    }
}

impl LandlockConfig {
    pub const SYNTAX: &'static str = "Landlock parameters \
        \"path=<path/to/{file/dir}>,access=[rw]\"";

    pub fn parse(landlock_rule: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add("path").add("access");
        parser
            .parse(landlock_rule)
            .map_err(Error::ParseLandlockRules)?;

        let path = parser
            .get("path")
            .map(PathBuf::from)
            .ok_or(Error::ParseLandlockMissingFields)?;

        let access = parser
            .get("access")
            .ok_or(Error::ParseLandlockMissingFields)?;

        if access.chars().count() > 2 {
            return Err(Error::ParseLandlockRules(OptionParserError::InvalidValue(
                access.to_string(),
            )));
        }

        Ok(LandlockConfig { path, access })
    }
}

#[cfg(feature = "ivshmem")]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct IvshmemConfig {
    #[serde(flatten)]
    pub pci_common: PciDeviceCommonConfig,
    pub path: PathBuf,
    pub size: usize,
}

#[cfg(feature = "ivshmem")]
impl From<IvshmemConfig> for vm_config::IvshmemConfig {
    fn from(value: IvshmemConfig) -> Self {
        Self {
            pci_common: value.pci_common.into(),
            path: value.path,
            size: value.size,
        }
    }
}

#[cfg(feature = "ivshmem")]
impl From<&vm_config::IvshmemConfig> for IvshmemConfig {
    fn from(value: &vm_config::IvshmemConfig) -> Self {
        Self {
            pci_common: (&value.pci_common).into(),
            path: value.path.clone(),
            size: value.size,
        }
    }
}

#[cfg(feature = "ivshmem")]
impl IvshmemConfig {
    pub const SYNTAX: &'static str = "Ivshmem device. Specify the backend file path and size \
    for the shared memory: \"path=</path/to/a/file>,size=<file_size>,id=<device_id>,\
    pci_segment=<segment_id>,pci_device_id=<pci_slot>\" \
    \nThe <file_size> must be a power of 2 (e.g., 2M, 4M, etc.), as it represents the size \
    of the memory region mapped to the guest. Default size is 128M.";

    pub fn parse(ivshmem: &str) -> Result<Self, Error> {
        let mut parser = OptionParser::new();
        parser.add("path").add("size");
        parser.add_all(PciDeviceCommonConfig::OPTIONS);
        parser.parse(ivshmem).map_err(Error::ParseIvshmem)?;
        let path = parser
            .get("path")
            .map(PathBuf::from)
            .ok_or(Error::ParseIvshmemPathMissing)?;
        let size = parser
            .convert::<ByteSized>("size")
            .map_err(Error::ParseIvshmem)?
            .unwrap_or(ByteSized((DEFAULT_IVSHMEM_SIZE << 20) as u64))
            .0;
        let pci_common = PciDeviceCommonConfig::parse(ivshmem)?;
        Ok(IvshmemConfig {
            pci_common,
            path,
            size: size as usize,
        })
    }
}
