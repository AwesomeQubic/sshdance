use std::borrow::Cow;
use std::fmt::Display;
use std::future::pending;
use std::io::Write;
use std::mem::replace;
use std::panic;
use std::panic::AssertUnwindSafe;

use crate::handle::SshTerminal;
use crate::handle::TerminalHandle;
use crate::site::Code;
use crate::site::DrawFunc;
use crate::site::DummyPage;
use crate::site::EscapeCode;
use crate::site::PageHandler;
use crate::site::SshInput;
use crate::site::SshPage;
use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use crossterm::cursor;
use crossterm::terminal;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::Command;
use crossterm::ExecutableCommand;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::TerminalOptions;
use ratatui::Viewport;
use russh::server::Auth;
use russh::server::Handler;
use russh::server::Msg;
use russh::server::Session;
use russh::Channel;
use russh::ChannelId;
use russh::Pty;
use tokio::sync::mpsc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::Interval;
use tracing::error;
use tracing::info;
use tracing::info_span;
use tracing::trace;
use tracing::warn;
use tracing_futures::Instrument;

pub struct ClientHandler<T: PageHandler + Send + Sync> {
    handler: T,
    state: State,
}

#[derive(Default)]
struct State {
    render_send: Option<UnboundedSender<DisplayMessage>>,
    render: Option<JoinHandle<()>>,
    main_chanel: Option<ChannelId>,
    animation: Option<Interval>,
}

enum DisplayMessage {
    Render(Option<Box<DrawFunc>>),
    Resize(Rect),
    SetTitle(Cow<'static, str>),
    Terminate(Option<Cow<'static, str>>),
}

struct DisplayTask {
    rx: UnboundedReceiver<DisplayMessage>,
    term: SshTerminal,
}

impl DisplayTask {
    async fn run(&mut self) -> anyhow::Result<()> {
        const MAX_SKIP: usize = 5;
        let mut buf = Vec::with_capacity(MAX_SKIP);
        loop {
            let read = self.rx.recv_many(&mut buf, MAX_SKIP).await;
            let read_slice = &mut buf[0..read];

            let mut first_resize = Option::None;
            let mut first_draw = Option::None;
            let mut last_resize = Option::None;

            let mut set_title = false;
            for message in read_slice.iter_mut().rev() {
                match message {
                    DisplayMessage::Render(fn_once) => {
                                        if first_draw.is_none() {
                                            first_draw = fn_once.take();
                                        }
                                    }
                    DisplayMessage::Resize(rect) => {
                                        if first_draw.is_none() {
                                            if first_resize.is_none() {
                                                first_resize = Some(rect);
                                            }
                                        } else {
                                            if last_resize.is_none() {
                                                last_resize = Some(rect);
                                            }
                                        }
                                    }
                    DisplayMessage::Terminate(cow) => {
                                        //TODO make it less hacky
                                        self.term.show_cursor()?;
                                        let backend = self.term.backend_mut();

                                        //backend.execute(DisableMouseCapture)?;
                                        backend.execute(cursor::Show)?;
                                        backend.execute(LeaveAlternateScreen)?;
                                        if let Some(cow) = cow.as_mut() {
                                            backend.write(cow.as_bytes());
                                        }
                                        return Ok(());
                                    }
                    DisplayMessage::SetTitle(cow) => {
                        if !set_title {
                            set_title = true;

                            let backend = self.term.backend_mut();
                            backend.execute(terminal::SetTitle(cow))?;
                        }
                    },
                }
            }

            if let Some(resize) = last_resize {
                self.term.resize(*resize);
            }

            if let Some(resize) = first_draw {
                self.term.draw(move |x| resize(x));
            }

            if let Some(resize) = first_resize {
                self.term.resize(*resize);
            }
        }
    }
}

impl<T: PageHandler> ClientHandler<T> {
    pub fn new(ip: Option<std::net::SocketAddr>) -> ClientHandler<T> {
        Self {
            handler: T::create(ip),
            state: State::default(),
        }
    }
}

impl<T: PageHandler + Sync + Send> Handler for ClientHandler<T> {
    type Error = anyhow::Error;

    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        info!("Doing auth for user {user}");
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        
        Ok(true)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        trace!("User input {data:?}");

        self.handler.handle_raw_input(data, &mut self.state);

        Ok(())
    }

    /// The client's pseudo-terminal window size has changed.
    async fn window_change_request(
        &mut self,
        _: ChannelId,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        if let Some(x) = &mut self.state.render_send {
            x.send(DisplayMessage::Resize(rect));
        };

        Ok(())
    }

    #[allow(unused_variables, clippy::too_many_arguments)]
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        pix_width: u32,
        pix_height: u32,
        modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Client is using {term}");
        if let Some(x) = &mut self.state.render_send {
            x.send(DisplayMessage::Resize(rect));
        };

        let terminal_handle = TerminalHandle::new(session.handle(), channel.id());

        let mut backend = CrosstermBackend::new(terminal_handle);
        backend.execute(EnterAlternateScreen)?;
        backend.execute(cursor::Hide)?;

        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let (tx, rx) = unbounded_channel();

        tokio::spawn(DisplayTask {
            rx,
            term: Terminal::with_options(
                backend,
                TerminalOptions {
                    viewport: Viewport::Fixed(rect),
                },
            )?
        }.run());

        self.state.render_send = Some(tx);
        self.handler.setup(&mut self.state);

        Ok(())
    }
}

async fn animation_interval(interval: Option<&mut Interval>) -> Instant {
    if let Some(animation) = interval {
        animation.tick().await
    } else {
        pending::<Instant>().await
    }
}

struct LoadedPage {
    animation: Option<Interval>,
    page: SshPage,
}

impl LoadedPage {

}

impl From<SshPage> for LoadedPage {
    fn from(page: SshPage) -> Self {
        let animation = if let Some(tps) = page.get_tps() {
            let delay = 1000 / tps;
            Some(interval(Duration::from_millis(delay as u64)))
        } else {
            None
        };

        LoadedPage { animation, page }
    }
}

impl ClientTask {
    async fn run(mut self) {
        loop {
            let code = if self.term.is_none() {
                let Some(event) = self.rx.recv().await else {
                    return;
                };

                self.handle_input(event).await
            } else {
                let rx_future = self.rx.recv();
                let anim_future = self.page.animation_interval();
                tokio::select! {
                    message = rx_future => {
                        let Some(event) = message else {
                            return;
                        };

                        self.handle_input(event).await
                    },
                    _anim = anim_future => {
                        self.page.page.tick()
                    }
                }
            };
        }
    }

    async fn terminate(mut self) -> Result<()> {
        trace!("Terminating connection to client");
        if let Some(mut term) = replace(&mut self.term, None) {
            //TODO make it less hacky
            term.show_cursor()?;

            let backend = term.backend_mut();
            //backend.execute(DisableMouseCapture)?;
            backend.execute(cursor::Show)?;
            backend.execute(LeaveAlternateScreen)?;
        }
        Ok(())
    }

    async fn handle_input(&mut self, message: ThreadMessage) -> Result<Code> {
        trace!("Handing input {message}");
        match message {
            ThreadMessage::NewTerm(backend, id) => {
                info!("Creating term");

                self.term = Some(Terminal::with_options(
                    backend,
                    TerminalOptions {
                        viewport: Viewport::Fixed(Rect {
                            x: 0,
                            y: 0,
                            width: 200,
                            height: 40,
                        }),
                    },
                )?);

                self.main_chanel = Some(id);
                Ok(Code::Render)
            }
            ThreadMessage::Resize(rect) => {
                info!("Resizing term {rect}");
                self.term
                    .as_mut()
                    .context("No term initialized")?
                    .resize(rect)?;
                Ok(Code::Render)
            }
            ThreadMessage::Input(SshInput::Special(EscapeCode::CtrlC)) => Ok(Code::Terminate),
            ThreadMessage::Input(x) => return self.page.page.handle_input(x).await,
        }
    }

    async fn render(mut self) -> RenderResult {
        trace!("Redrawing terminal");
        let back: RenderResult = tokio::task::spawn_blocking(move || {
            let self_mut = &mut self;
            //TODO prove unwind safety of renderer and term

            let Some(term) = self_mut.term.as_mut() else {
                unreachable!("You can not call render on non exist page");
            };

            let renderer = &mut *self_mut.page.page;

            let out = panic::catch_unwind(AssertUnwindSafe(|| {
                term.draw(move |frame| {
                    let area = frame.area();
                    renderer.render(frame, area);
                })
            }));

            let processed = match out {
                std::result::Result::Ok(std::io::Result::Ok(_)) => std::result::Result::Ok(()),
                std::result::Result::Ok(std::io::Result::Err(e)) => {
                    std::result::Result::Err(RenderError::InternalError(e))
                }
                Err(_) => {
                    if let Some(backend) = self_mut.term.as_mut() {
                        backend.backend_mut().writer_mut().post_panic();
                    }
                    self_mut.page = LoadedPage::from(Box::new(DummyPage) as SshPage);
                    error!("SSH website panicked");
                    std::result::Result::Err(RenderError::Panicked)
                }
            };

            RenderResult {
                ownership: self,
                results: processed,
            }
        })
        .await
        .unwrap(); //Unwrap is safe since all possibilities of a panic are checked

        back
    }
}

struct RenderResult {
    ownership: ClientTask,
    results: std::result::Result<(), RenderError>,
}

enum RenderError {
    Panicked,
    InternalError(std::io::Error),
}
