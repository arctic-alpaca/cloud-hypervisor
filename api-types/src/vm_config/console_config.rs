use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ConsoleOutputMode {
    Off,
    Pty,
    Tty,
    File,
    Socket,
    Null,
}
