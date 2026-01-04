use std::mem::replace;
use ratatui::{prelude::CrosstermBackend, Terminal};
use russh::{ChannelId, CryptoVec, server::Handle};
use tokio::{sync::mpsc::{UnboundedSender, unbounded_channel}, task::JoinHandle};
use tracing::{trace, warn};


pub type RatatuiTerminal = Terminal<CrosstermBackend<SinkTerminalHandle>>;

pub struct SinkTerminalHandle {
    // The sink collects the data which is finally flushed to the handle.
    sink: CryptoVec,

    tx: UnboundedSender<WriteMessage>,
    handle: Option<JoinHandle<()>>,
}

enum WriteMessage {
    Close,
    Write(CryptoVec),
}

impl Drop for SinkTerminalHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.as_mut() {
            warn!("Ungracefully shuting down connection");
            handle.abort();
        }
    }
}

impl SinkTerminalHandle {
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

        SinkTerminalHandle {
            sink: CryptoVec::new(),
            tx,
            handle: Some(handle),
        }
    }

    pub async fn close(&mut self) -> Result<(), crate::Error> {
        self.tx.send(WriteMessage::Close).unwrap();

        let mut handle_option = replace(&mut self.handle, None);
        let handle = handle_option
            .as_mut().unwrap();
        handle.await.unwrap();

        Ok(())
    }
}

// The crossterm backend writes to the terminal handle.
impl std::io::Write for SinkTerminalHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let old_vec = replace(&mut self.sink, CryptoVec::new());
        if let Err(_) = self.tx.send(WriteMessage::Write(old_vec)) {
            return std::io::Result::Err(std::io::ErrorKind::BrokenPipe.into());
        };
        Ok(())
    }
}