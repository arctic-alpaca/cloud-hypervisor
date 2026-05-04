use std::os::fd::RawFd;

use serializable_fd::SerializableFd;

use crate::private_trait::Parseable;
use crate::{IntegerList, TupleError, TupleValue};

impl TupleValue for Vec<SerializableFd> {
    fn parse_value(input: &str) -> Result<Self, TupleError>
    where
        Self: Sized,
    {
        Ok(IntegerList::from_str(input)
            .map_err(TupleError::InvalidIntegerList)?
            .0
            .iter()
            .map(|raw_fd| {
                // SAFETY: TODO
                unsafe { SerializableFd::new_valid_from_raw(*raw_fd as RawFd) }
            })
            .collect())
    }
}

// impl Parseable for SerializableFd {
//     type Err = <i32 as FromStr>::Err;
//
//     fn from_str(input: &str) -> Result<Self, <Self as Parseable>::Err> {
//         let raw_fd = <i32 as Parseable>::from_str(input)?;
//
//         // SAFETY: TODO
//         let valid_fd = unsafe { OwnedFd::from_raw_fd(raw_fd as RawFd) };
//         Ok(SerializableFd::new_valid(valid_fd))
//     }
// }
