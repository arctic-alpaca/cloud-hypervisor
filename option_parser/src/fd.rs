use std::fs::File;
use std::ops::Deref;
use std::os::fd::{AsRawFd, RawFd};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::private_trait::Parseable;
use crate::{IntegerList, TupleError, TupleValue};

#[derive(Error, Debug, Eq, PartialEq)]
pub enum FdDeviceParseError {
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize, Ord, PartialOrd)]
pub enum FdDevice {
    Net { id: String },
}

impl Parseable for FdDevice {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerializableFdInner {
    Valid(RawFd),
    Invalid(RawFd),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializableFd {
    #[serde(
        serialize_with = "serialize_serializable_fd_inner",
        deserialize_with = "deserialize_serializable_fd_inner"
    )]
    inner: SerializableFdInner,
}

impl SerializableFd {
    pub fn new_valid(fd: RawFd) -> Self {
        Self {
            inner: SerializableFdInner::Valid(fd),
        }
    }
    pub fn new_invalid(fd: RawFd) -> Self {
        Self {
            inner: SerializableFdInner::Invalid(fd),
        }
    }

    pub fn update_fds(serializable_fds: &mut [Self], valid_fds: Vec<File>) {
        // TODO: proper error handling
        assert_eq!(serializable_fds.len(), valid_fds.len());
        serializable_fds
            .iter_mut()
            .zip(valid_fds)
            .for_each(|(serializable_fd, fd)| serializable_fd.update_fd(fd.as_raw_fd()));
    }
}

impl Deref for SerializableFd {
    type Target = RawFd;

    fn deref(&self) -> &Self::Target {
        match &self.inner {
            SerializableFdInner::Valid(fd) => fd,
            SerializableFdInner::Invalid(_) => {
                panic!("cannot access invalid FD");
            }
        }
    }
}

impl SerializableFd {
    pub fn update_fd(&mut self, fd: RawFd) {
        if let SerializableFdInner::Valid(_) = self.inner {}
        match self.inner {
            SerializableFdInner::Valid(_) => {
                // TODO: proper error handling
                panic!("Cannot update valid FD");
            }
            SerializableFdInner::Invalid(_) => {
                self.inner = SerializableFdInner::Valid(fd);
            }
        }
    }
}

fn deserialize_serializable_fd_inner<'de, D>(d: D) -> Result<SerializableFdInner, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let fd: SerializableFdInner = SerializableFdInner::deserialize(d)?;
    match fd {
        SerializableFdInner::Valid(fd) => Ok(SerializableFdInner::Invalid(fd)),
        SerializableFdInner::Invalid(fd) => Ok(SerializableFdInner::Invalid(fd)),
    }
}

fn serialize_serializable_fd_inner<S>(
    serializable_fd_inner: &SerializableFdInner,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let fd = match serializable_fd_inner {
        SerializableFdInner::Valid(fd) => SerializableFdInner::Invalid(*fd),
        SerializableFdInner::Invalid(fd) => SerializableFdInner::Invalid(*fd),
    };
    fd.serialize(serializer)
}

// impl Parseable for SerializableFd {
//     type Err = ();
//
//     fn from_str(input: &str) -> Result<Self, <Self as Parseable>::Err> {
//         //TODO: error handling
//         let fd = <i32 as Parseable>::from_str(input).unwrap();
//         Ok(Self::new_valid(fd))
//     }
// }

impl TupleValue for Vec<SerializableFd> {
    fn parse_value(input: &str) -> Result<Self, TupleError>
    where
        Self: Sized,
    {
        Ok(IntegerList::from_str(input)
            .map_err(TupleError::InvalidIntegerList)?
            .0
            .iter()
            .map(|v| SerializableFd::new_valid(*v as RawFd))
            .collect())
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
