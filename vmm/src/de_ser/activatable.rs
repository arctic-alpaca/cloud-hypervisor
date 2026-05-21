use std::os::fd::OwnedFd;

use thiserror::Error;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum Error {
    //TODO(de_ser)
    #[error("todo")]
    Todo,
}

pub trait Activatable {
    type Activated;
    fn activate(self, fds: Vec<OwnedFd>) -> Result<Self::Activated, Error>;
    fn fd_list(&self) -> Vec<OwnedFd>;
}
