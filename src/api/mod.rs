use std::net::SocketAddr;

use crate::api::term::SshTerminal;

pub mod term;
pub mod utils;

/// Session controller for ssh dance
pub trait ClientHandler: Sync + Send + 'static {
    type TerminalHandler: SshTerminal;

    fn create(addr: Option<SocketAddr>) -> Self;

    fn terminal_request(&mut self) -> Decision {
        Decision::Accept
    }

    fn new_terminal(&mut self) -> Self::TerminalHandler;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Decision {
    Accept,
    Deny,
}
