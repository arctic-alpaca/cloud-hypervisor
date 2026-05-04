use std::fs::File;
use std::mem;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::str::FromStr;

use serde::de::Error;
use serde::{Deserialize, Serialize, Serializer};
use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum FdDeviceParseError {
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize, Ord, PartialOrd)]
pub enum FdDevice {
    Net { id: String },
}

#[derive(Debug, Deserialize)]
pub enum SerializableFd {
    #[serde(deserialize_with = "deserialize_serializable_fd")]
    Valid(OwnedFd),
    Invalid(RawFd),
}

impl Eq for SerializableFd {}

impl PartialEq for SerializableFd {
    fn eq(&self, other: &Self) -> bool {
        match self {
            SerializableFd::Valid(self_fd) => match other {
                SerializableFd::Valid(other_fd) => self_fd.as_raw_fd() == other_fd.as_raw_fd(),
                SerializableFd::Invalid(_) => false,
            },
            SerializableFd::Invalid(self_fd) => match other {
                SerializableFd::Valid(_) => false,
                SerializableFd::Invalid(other_fd) => self_fd == other_fd,
            },
        }
    }
}

impl From<SerializableFd> for OwnedFd {
    fn from(value: SerializableFd) -> Self {
        match value {
            SerializableFd::Valid(fd) => fd,
            SerializableFd::Invalid(_) => {
                panic!("cannot access invalid FD");
            }
        }
    }
}

impl AsFd for SerializableFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        match &self {
            SerializableFd::Valid(fd) => fd.as_fd(),
            SerializableFd::Invalid(_) => {
                panic!("cannot access invalid FD");
            }
        }
    }
}

impl AsRawFd for SerializableFd {
    fn as_raw_fd(&self) -> RawFd {
        match &self {
            SerializableFd::Valid(fd) => fd.as_raw_fd(),
            SerializableFd::Invalid(_) => {
                panic!("cannot access invalid FD");
            }
        }
    }
}

fn deserialize_serializable_fd<'de, D>(_d: D) -> Result<OwnedFd, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Err(D::Error::custom("deserializing valid FD is not allowed"))
}

impl Serialize for SerializableFd {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            SerializableFd::Valid(ref fd) => S::serialize_newtype_variant(
                serializer,
                "SerializableFd",
                1u32,
                "Invalid",
                &fd.as_raw_fd(),
            ),
            SerializableFd::Invalid(ref fd) => {
                S::serialize_newtype_variant(serializer, "SerializableFd", 1u32, "Invalid", fd)
            }
        }
    }
}

impl Clone for SerializableFd {
    fn clone(&self) -> Self {
        match self {
            SerializableFd::Valid(fd) => {
                let duplicated_fd = fd.try_clone().unwrap();
                Self::Valid(duplicated_fd)
            }
            SerializableFd::Invalid(fd) => Self::Invalid(*fd),
        }
    }
}

impl SerializableFd {
    pub fn is_valid(&self) -> bool {
        match self {
            SerializableFd::Valid(_) => true,
            SerializableFd::Invalid(_) => false,
        }
    }

    pub fn new_valid(fd: OwnedFd) -> Self {
        SerializableFd::Valid(fd)
    }

    pub fn extract_fd(&mut self) -> Option<Self> {
        if let SerializableFd::Valid(_) = self {
            let fd = self.as_raw_fd();
            let mut swap = Self::Invalid(fd);
            mem::swap(self, &mut swap);
            Some(swap)
        } else {
            None
        }
    }

    /// # Safety
    /// TODO
    pub unsafe fn new_valid_from_raw(fd: RawFd) -> Self {
        // TODO: error handling
        assert!(fd >= 1, "invalid FD");
        // SAFETY: TODO
        let valid_fd = unsafe { OwnedFd::from_raw_fd(fd) };
        Self::new_valid(valid_fd)
    }

    pub fn new_invalid(fd: RawFd) -> Self {
        SerializableFd::Invalid(fd)
    }

    pub fn update_fds(serializable_fds: &mut [Self], valid_fds: Vec<File>) {
        // TODO: proper error handling
        assert_eq!(serializable_fds.len(), valid_fds.len());
        serializable_fds
            .iter_mut()
            .zip(valid_fds)
            .for_each(|(serializable_fd, file)| serializable_fd.update_fd(OwnedFd::from(file)));
    }

    pub fn update_fd(&mut self, fd: OwnedFd) {
        match self {
            SerializableFd::Valid(_) => {
                // TODO: proper error handling
                panic!("Cannot update valid FD");
            }
            SerializableFd::Invalid(_) => {
                *self = SerializableFd::Valid(fd);
            }
        }
    }
}

impl FromStr for FdDevice {
    type Err = FdDeviceParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let parts = s
            .split_once('(')
            .ok_or(FdDeviceParseError::InvalidValue(s.to_owned()))?;
        let inner_value = &parts.1[0..parts.1.len() - 1];
        let expected_closing_bracket = &parts.1[parts.1.len() - 1..];
        let result = match parts.0 {
            "net" => Ok(FdDevice::Net {
                id: inner_value
                    .parse::<usize>()
                    .map_err(|_| FdDeviceParseError::InvalidValue(inner_value.to_owned()))?
                    .to_string(),
            }),
            unknown => Err(FdDeviceParseError::InvalidValue(unknown.to_owned())),
        }?;
        if expected_closing_bracket != ")" {
            return Err(FdDeviceParseError::InvalidValue(s.to_owned()));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_parse_valid_net_device() {
        let input = "net(123)";
        assert_eq!(
            FdDevice::from_str(input),
            Ok(FdDevice::Net {
                id: "123".to_owned()
            })
        );

        let input = "net(-123)";
        assert_eq!(
            FdDevice::from_str(input),
            Err(FdDeviceParseError::InvalidValue("-123".to_owned()))
        );

        let input = "foo(123)";
        assert_eq!(
            FdDevice::from_str(input),
            Err(FdDeviceParseError::InvalidValue("foo".to_owned()))
        );

        let input = "net(123";
        assert_eq!(
            FdDevice::from_str(input),
            Err(FdDeviceParseError::InvalidValue("net(123".to_owned()))
        );
    }
}
