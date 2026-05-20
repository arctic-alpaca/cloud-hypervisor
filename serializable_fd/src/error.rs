use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum Error {
    //TODO(de_ser)
    #[error("todo")]
    Todo,
}
