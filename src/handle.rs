use std::{io::Write as _, mem::replace};

use anyhow::Context;
use ratatui::{prelude::CrosstermBackend, Terminal};
use russh::{server::Handle, ChannelId, CryptoVec};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::JoinHandle,
};
use tracing::{trace, warn};

pub type SshTerminal = Terminal<CrosstermBackend<TerminalHandle>>;

pub struct TerminalHandle {
    // The sink collects the data which is finally flushed to the handle.
    sink: CryptoVec,

    tx: UnboundedSender<WriteMessage>,
    handle: Option<JoinHandle<()>>,
}

enum WriteMessage {
    Close,
    Write(CryptoVec),
}

impl Drop for  TerminalHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.as_mut() {
            warn!("Ungracefully shuting down connection");
            handle.abort();
        }
    }
}

impl TerminalHandle {
    pub fn new(handle: Handle, channel_id: ChannelId) -> Self {
        let (tx, mut rx) = unbounded_channel::<WriteMessage>();
        let handle = tokio::spawn(async move {
            loop {
                let Some(data) = rx.recv().await else {
                    warn!("Encountered an error while receiving write message message");
                    continue;
                };

                match data {
                    WriteMessage::Close => {
                        trace!("Closing session with client");
                        if let Err(_) = handle.close(channel_id).await {
                            warn!("Encounter error while terminating connection")
                        };
                        return;
                    }
                    WriteMessage::Write(data) => {
                        trace!("Sending data to client");
                        if let Err(err) = handle.data(channel_id, data).await {
                            warn!("Encounter error {err:?} while sending data to connection")
                        };
                    }
                }
            }
        });

        TerminalHandle {
            sink: CryptoVec::new(),
            tx,
            handle: Some(handle),
        }
    }

    pub async fn close(&mut self) -> anyhow::Result<()>{
        self.tx.send(WriteMessage::Close)?;

        let mut handle_option = replace(&mut self.handle, None);
        let handle = handle_option.as_mut().with_context(|| "Closing a already closed connection")?;
        handle.await?;

        anyhow::Ok(())
    }
}

// The crossterm backend writes to the terminal handle.
impl std::io::Write for TerminalHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let old_vec = replace(&mut self.sink, CryptoVec::new());
        if let Err(_) = self.tx.send(WriteMessage::Write(old_vec)) {
            warn!("Error sending message to task but rusts stupidly does not let us represent that so idk have fun");
        };
        Ok(())
    }
}
