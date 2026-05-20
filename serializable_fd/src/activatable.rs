use crate::error::Error;
use crate::{Active, Fd};

pub trait Activatable {
    type Activated;
    type Ingest;
    fn activate(self, ingest: &mut Self::Ingest) -> Result<Self::Activated, Error>;
}

pub trait Serializable {
    fn fd_list(&self, fds: &mut Vec<Fd<Active>>);
}
