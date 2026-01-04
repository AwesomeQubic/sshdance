use std::{future::pending, io::Write, marker::PhantomData, time::Duration};

use crossterm::{
    cursor,
    terminal::{Clear, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{layout::Rect, prelude::CrosstermBackend, TerminalOptions};
use russh::{ChannelId, server::Handle};
use termwiz::input::InputParser;
use tokio::{
    select,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
    time::{interval, Instant, Interval},
};
use tracing::{debug, info, trace, warn};

use crate::{
    api::{
        term::{CallbackRez, EngineRef, SshTerminal},
        ClientHandler,
    },
    internal::sync_sink::{self, RatatuiTerminal},
};

//I hate this but its the most way sane way to not block main thread
pub struct RenderEngineApi<T: SshTerminal> {
    async_notifs_tx: UnboundedSender<T::MessageType>,
    async_notifs_rx: UnboundedReceiver<T::MessageType>,

    phantom: PhantomData<T>,

    size: Rect,

    anim: Option<Interval>,
}

impl<T: SshTerminal> RenderEngineApi<T> {
    pub fn create(size: Rect) -> Self {
        let (ntx, nrx) = unbounded_channel();
        Self {
            phantom: PhantomData,
            async_notifs_tx: ntx,
            async_notifs_rx: nrx,
            anim: T::DEFAULT_TPS
                .map(|x| 1.0 / (x.get() as f32))
                .map(|x| interval(Duration::from_secs_f32(x))),
            size,
        }
    }
}

impl<T: SshTerminal> EngineRef<T> for RenderEngineApi<T> {
    fn terminal_channel(&mut self) -> UnboundedSender<<T as SshTerminal>::MessageType> {
        self.async_notifs_tx.clone()
    }

    fn current_size(&mut self) -> Rect {
        self.size.clone()
    }
}

pub enum TerminalInputs {
    Resize((u32, u32)),
    Input(termwiz::input::InputEvent),
}

pub async fn create_and_detach<H: ClientHandler>(
    channel: russh::Channel<russh::server::Msg>,
    width: u32,
    height: u32,
    session_handler: &mut H,
    handle: Handle,
    channel_id: ChannelId
) -> Result<(UnboundedSender<TerminalInputs>, JoinHandle<()>, InputParser), crate::Error> {
    let mut backend = CrosstermBackend::new(sync_sink::SinkTerminalHandle::new(handle, channel_id));
    backend.execute(EnterAlternateScreen)?;
    backend.execute(cursor::Hide)?;
    backend.execute(Clear(crossterm::terminal::ClearType::All))?;

    let term = RatatuiTerminal::with_options(
        backend,
        TerminalOptions {
            viewport: ratatui::Viewport::Fixed(Rect {
                x: 0,
                y: 0,
                width: width as u16,
                height: height as u16,
            }),
        },
    )?;

    let (sender, receiver) = unbounded_channel();
    let handler_term = session_handler.new_terminal();
    let join_handle = tokio::task::spawn(dispatch::<H>(receiver, handler_term, term));
    Ok((sender, join_handle, InputParser::new()))
}

async fn dispatch<H: ClientHandler>(
    input: UnboundedReceiver<TerminalInputs>,
    handler: H::TerminalHandler,
    term: RatatuiTerminal,
) {
    debug!("Dispatching new terminal session");
    let Err(error) = dispatch_inner::<H>(input, handler, term).await else {
        info!("Session ended without errors");
        return;
    };

    warn!("Error while handling session {error:?}");
}

async fn dispatch_inner<H: ClientHandler>(
    mut input: UnboundedReceiver<TerminalInputs>,
    mut handler: H::TerminalHandler,
    mut term: RatatuiTerminal,
) -> Result<(), crate::Error> {
    let mut engine: RenderEngineApi<H::TerminalHandler> =
        RenderEngineApi::create(term.get_frame().area());
    let mut recv_buf = Vec::new();
    loop {
        trace!("New client wait loop");
        recv_buf.clear();
        let state = select! {
            rez = input.recv_many(&mut recv_buf, 20) => {
                trace!("Handler wake {rez:?}");
                if rez == 0 {
                    return Err(crate::Error::SessionClosed);
                }

                let mut current_state = CallbackRez::Continue;
                for i in recv_buf.iter().rev() {
                    let TerminalInputs::Resize((width, height)) = i else {
                        continue;
                    };

                    let width = *width as u16;
                    let height = *height as u16;
                    let rect = Rect { x: 0, y: 0, width, height };
                    term.resize(rect)?;
                    current_state = current_state.pick(handler.on_resize(&mut engine, width , height));
                    engine.size = rect;
                    break;
                }

                while let Some(i) = recv_buf.pop() {
                    let TerminalInputs::Input(input) = i else {
                        continue;
                    };
                    current_state = current_state.pick(handler.on_input(&mut engine, input));
                }

                current_state
            },
            rez = engine.async_notifs_rx.recv() => {
                let Some(out) = rez else {
                    continue;
                };
                handler.on_message(&mut engine, out)
            },
            anim = animation_interval(&mut engine.anim) => {
                trace!("Animation wake {anim:?}");
                handler.on_animation(&mut engine)
            }
        };

        trace!("Loop result: {state:?}");

        match state {
            CallbackRez::PushToRenderer => {
                let inner = &mut handler;
                //TODO: benchmark if block_in_place would make a difference
                let rez = term.draw(move |x| {
                    inner.draw(x);
                });
                trace!("Frame finished");
                if let Err(error) = rez {
                    warn!("Error while rendering: {error:?}");
                }
            }
            CallbackRez::Terminate(msg) => {
                //TODO make it less hacky
                term.show_cursor().unwrap();

                let backend = term.backend_mut();
                backend.execute(cursor::Show)?;
                backend.execute(LeaveAlternateScreen)?;

                backend.write(msg.replace("\n", "\n\r").as_bytes())?;
                backend.write(b"\n\r")?;
                backend.flush()?;

                backend.writer_mut().close().await?;

                return Ok(());
            }
            _ => {}
        }
    }
}

async fn animation_interval(interval: &mut Option<Interval>) -> Instant {
    if let Some(animation) = interval.as_mut() {
        animation.tick().await
    } else {
        pending::<Instant>().await
    }
}
