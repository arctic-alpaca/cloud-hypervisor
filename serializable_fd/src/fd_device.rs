use std::str::FromStr;

use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum FdDeviceParseError {
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
pub enum FdDevice {
    Net { id: String },
}

impl FdDevice {
    pub fn new_net(id: String) -> Self {
        Self::Net { id }
    }
}

impl FromStr for FdDevice {
    type Err = FdDeviceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (device, rest) = s
            .split_once('(')
            .ok_or(FdDeviceParseError::InvalidValue(s.to_owned()))?;
        let metadata = &rest[0..rest.len() - 1];
        let expected_closing_bracket = &rest[rest.len() - 1..];
        let fd_device = match device {
            "net" => Ok(FdDevice::Net {
                id: metadata.to_string(),
            }),
            unknown => Err(FdDeviceParseError::InvalidValue(unknown.to_owned())),
        }?;
        if expected_closing_bracket != ")" {
            return Err(FdDeviceParseError::InvalidValue(s.to_owned()));
        }
        Ok(fd_device)
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_from_str() {
        let input = "net(foo_123!?())";
        assert_eq!(
            FdDevice::from_str(input),
            Ok(FdDevice::Net {
                id: "foo_123!?()".to_owned()
            })
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
