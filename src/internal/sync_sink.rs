use std::{io::Write as _, mem::replace};

use ratatui::{prelude::CrosstermBackend, Terminal};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tracing::{trace, warn};

pub type RatatuiTerminal = Terminal<CrosstermBackend<SinkTerminalHandle>>;

pub struct SinkTerminalHandle {
    // The sink collects the data which is finally flushed to the handle.
    sink: Vec<u8>,
    tx: UnboundedSender<WriteMessage>,
}

enum WriteMessage {
    Close,
    Write(Vec<u8>),
}

impl Drop for SinkTerminalHandle {
    fn drop(&mut self) {
        //Try flushing one last time
        let _ = self.flush();
        //Close thread
        self.tx
            .send(WriteMessage::Close)
            .expect("Thread can not be closed yet");
    }
}

impl SinkTerminalHandle {
    pub fn new(channel: russh::Channel<russh::server::Msg>) -> Self {
        let (tx, mut rx) = unbounded_channel::<WriteMessage>();
        tokio::spawn(async move {
            loop {
                let Some(data) = rx.recv().await else {
                    warn!("Encountered an error while receiving write message message");
                    continue;
                };

                match data {
                    WriteMessage::Close => {
                        trace!("Closing session with client");
                        if let Err(_) = channel.close().await {
                            warn!("Encounter error while terminating connection")
                        };
                        return;
                    }
                    WriteMessage::Write(data) => {
                        if let Err(err) = channel.data(&*data).await {
                            warn!("Encounter error {err:?} while sending data to connection")
                        };
                    }
                }
            }
        });

        SinkTerminalHandle {
            sink: Vec::new(),
            tx,
        }
    }
}

// The crossterm backend writes to the terminal handle.
impl std::io::Write for SinkTerminalHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let old_vec = replace(&mut self.sink, Vec::new());
        if let Err(_) = self.tx.send(WriteMessage::Write(old_vec)) {
            return std::io::Result::Err(std::io::ErrorKind::BrokenPipe.into());
        };
        Ok(())
    }
}
