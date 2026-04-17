use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::private_trait::Parseable;

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
                id: inner_value.to_owned(),
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
