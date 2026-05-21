use std::collections::VecDeque;
use std::fmt::Debug;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::de_ser::activatable::FdList;
use crate::de_ser::status_marker::{Active, Serialized, StatusMarker};
use crate::de_ser::{Activatable, Error};

//TODO(de_ser): remove
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
#[serde(bound(deserialize = "Fd<S>: Deserialize<'de>"))]
pub struct VmConfig<S>
where
    S: StatusMarker + FdMarker,
{
    #[serde(default)]
    fds: Vec<Fd<S>>,
}

pub trait FdMarker: FdMarkerImpl {}

trait FdMarkerImpl {
    type InnerType: Debug + AsRawFd;
    fn clone(inner: &Self::InnerType) -> Self::InnerType;
}

impl FdMarkerImpl for Serialized {
    type InnerType = RawFd;

    fn clone(inner: &Self::InnerType) -> Self::InnerType {
        *inner
    }
}
impl FdMarker for Serialized {}

impl FdMarkerImpl for Active {
    type InnerType = OwnedFd;

    fn clone(inner: &Self::InnerType) -> Self::InnerType {
        inner.try_clone().unwrap()
    }
}
impl FdMarker for Active {}

#[derive(Debug)]
pub struct Fd<S>
where
    S: StatusMarker + FdMarker,
{
    fd: <S as FdMarkerImpl>::InnerType,
}

impl FdList for Fd<Active> {
    fn fd_list(&self, fds: &mut Vec<OwnedFd>) {
        fds.push(self.fd.try_clone().unwrap());
    }
}

impl Activatable for Fd<Serialized> {
    type Activated = Fd<Active>;

    fn activate(self, fds: &mut VecDeque<OwnedFd>) -> Result<Self::Activated, Error> {
        Ok(Fd {
            fd: fds.pop_front().unwrap(),
        })
    }
}

impl Fd<Serialized> {
    pub fn new(raw_fd: RawFd) -> Self {
        Self { fd: raw_fd }
    }
}

impl Fd<Active> {
    pub fn new(owned_fd: OwnedFd) -> Self {
        Self { fd: owned_fd }
    }
}

impl Default for Fd<Serialized> {
    fn default() -> Self {
        Self { fd: -1 }
    }
}

impl<S> Clone for Fd<S>
where
    S: StatusMarker + FdMarker,
{
    fn clone(&self) -> Self {
        Self {
            fd: <S as FdMarkerImpl>::clone(&self.fd),
        }
    }
}

impl<S> Eq for Fd<S> where S: StatusMarker + FdMarker {}

impl<S> PartialEq for Fd<S>
where
    S: StatusMarker + FdMarker,
{
    fn eq(&self, other: &Self) -> bool {
        self.fd.as_raw_fd() == other.fd.as_raw_fd()
    }
}

impl<S> Serialize for Fd<S>
where
    S: StatusMarker + FdMarker,
{
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: Serializer,
    {
        serializer.serialize_i32(self.fd.as_raw_fd())
    }
}

impl<'de> Deserialize<'de> for Fd<Serialized> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fd = RawFd::deserialize(deserializer)?;
        Ok(Self::new(fd))
    }
}

impl AsFd for Fd<Active> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl AsRawFd for Fd<Active> {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl From<OwnedFd> for Fd<Active> {
    fn from(value: OwnedFd) -> Self {
        Self::new(value)
    }
}

impl From<Fd<Active>> for OwnedFd {
    fn from(value: Fd<Active>) -> Self {
        value.fd
    }
}
