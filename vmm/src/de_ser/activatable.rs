use std::collections::VecDeque;
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
    fn activate(self, fds: &mut VecDeque<OwnedFd>) -> Result<Self::Activated, Error>;
}

pub trait FdList {
    fn fd_list(&self, fds: &mut Vec<OwnedFd>);
}
