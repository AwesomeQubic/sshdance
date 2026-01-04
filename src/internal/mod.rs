use std::collections::HashMap;

use russh::{server::Handler, ChannelId};
use termwiz::input::InputParser;
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};
use tracing::trace;

use crate::{
    api::{ClientHandler, Decision},
    internal::term::TerminalInputs,
};

mod sync_sink;
mod term;

pub struct SshSessionHandler<T: ClientHandler> {
    handler: T,
    channels: HashMap<ChannelId, ChannelState>,
}

impl<T: ClientHandler> SshSessionHandler<T> {
    pub fn create(addr: Option<std::net::SocketAddr>) -> Self {
        SshSessionHandler {
            handler: T::create(addr),
            channels: HashMap::new(),
        }
    }
}

enum ChannelState {
    TerminalSession((UnboundedSender<TerminalInputs>, JoinHandle<()>, InputParser)),
}

impl<T: ClientHandler> Handler for SshSessionHandler<T> {
    type Error = crate::Error;


    async fn auth_none(&mut self, _user: &str) -> Result<russh::server::Auth, Self::Error> {
        Ok(russh::server::Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _channel: russh::Channel<russh::server::Msg>,
        _session: &mut russh::server::Session,
    ) -> Result<bool, Self::Error> {
        Ok(self.handler.terminal_request() == Decision::Accept)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {

        let session =
            term::create_and_detach(col_width, row_height, &mut self.handler, session.handle(), channel).await?;

        self.channels
            .insert(channel, ChannelState::TerminalSession(session));
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        let state = self
            .channels
            .get_mut(&channel)
            .ok_or(crate::Error::UnknownChannel)?;

        trace!("Got data from client {data:?}");

        match state {
            ChannelState::TerminalSession((sender, _, parser)) => {
                parser.parse(
                    data,
                    |x| sender.send(TerminalInputs::Input(x)).unwrap(),
                    true,
                );
            }
            _ => return Err(crate::Error::UnknownChannel),
        }

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        let state = self
            .channels
            .get_mut(&channel)
            .ok_or(crate::Error::UnknownChannel)?;

        match state {
            ChannelState::TerminalSession((sender, _, _)) => {
                sender
                    .send(TerminalInputs::Resize((col_width, row_height)))
                    .unwrap();
            }
            _ => return Err(crate::Error::UnknownChannel),
        }

        Ok(())
    }
}
