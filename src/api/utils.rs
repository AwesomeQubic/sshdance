use std::marker::PhantomData;

use crate::api::{term::SshTerminal, ClientHandler};

#[derive(Default)]
pub struct SimpleTerminalHandler<T: SshTerminal + Default>(PhantomData<T>);

impl<T: SshTerminal + Default + 'static> ClientHandler for SimpleTerminalHandler<T> {
    type TerminalHandler = T;

    fn new_terminal(&mut self) -> Self::TerminalHandler {
        T::default()
    }

    fn create(_addr: Option<std::net::SocketAddr>) -> Self {
        Self(PhantomData)
    }
}
