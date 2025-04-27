use std::fmt::Display;
use std::future::pending;
use std::mem::replace;
use std::panic;
use std::panic::AssertUnwindSafe;

use crate::handle::SshTerminal;
use crate::handle::TerminalHandle;
use crate::site::Code;
use crate::site::DummyPage;
use crate::site::EscapeCode;
use crate::site::SshInput;
use crate::site::SshPage;
use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use crossterm::cursor;
use crossterm::terminal;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::ExecutableCommand;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
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
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
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

pub struct ClientHandler {
    thread: JoinHandle<()>,
    tx: Sender<ThreadMessage>,
    window_title: Option<&'static str>,
}

impl Drop for ClientHandler {
    fn drop(&mut self) {
        info!("Closing connection...");
        self.thread.abort();
    }
}

impl ClientHandler {
    pub fn new(
        ip: Option<std::net::SocketAddr>,
        page: SshPage,
        window_title: Option<&'static str>,
    ) -> ClientHandler {
        let (tx, rx) = mpsc::channel::<ThreadMessage>(100);

        let ip_formatted = ip
            .map(|x| format!("{x}"))
            .unwrap_or_else(|| "N/A".to_string());

        let span = info_span!("Client Task", ip = ip_formatted);

        let task = ClientTask {
            rx,
            term: None,
            main_chanel: None,
            page: page.into(),
        }
        .run()
        .instrument(span);
        let thread = tokio::task::spawn(task);

        ClientHandler {
            thread,
            tx,
            window_title,
        }
    }
}

impl Handler for ClientHandler {
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
        session.handle();

        let terminal_handle = TerminalHandle::new(session.handle(), channel.id());

        let mut backend = CrosstermBackend::new(terminal_handle);

        backend.execute(EnterAlternateScreen)?;
        backend.execute(cursor::Hide)?;

        if let Some(title) = self.window_title {
            backend.execute(terminal::SetTitle(title))?;
        }

        self.tx
            .try_send(ThreadMessage::NewTerm(backend, channel.id()))?;

        Ok(true)
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        trace!("User input {data:?}");

        match data {
            [3] => {
                //Esc
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::Special(EscapeCode::CtrlC)))?
            }
            [27] => {
                //Esc
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::Special(EscapeCode::Esc)))?
            }
            [13] => {
                //Enter
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::Special(EscapeCode::Enter)))?
            }
            [27, 91, 65] => {
                //Up arrow
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::Special(EscapeCode::Up)))?
            }
            [27, 91, 66] => {
                //Down arrow
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::Special(EscapeCode::Down)))?
            }
            [27, 91, 67] => {
                //Right arrow
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::Special(EscapeCode::Right)))?
            }
            [27, 91, 68] => {
                //Left arrow
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::Special(EscapeCode::Left)))?
            }
            _ => {
                self.tx
                    .try_send(ThreadMessage::Input(SshInput::KeyPress(char::try_from(
                        data[0],
                    )?)))?;
            }
        }

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

        self.tx.try_send(ThreadMessage::Resize(rect))?;
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
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };
        self.tx.try_send(ThreadMessage::Resize(rect))?;
        Ok(())
    }
}

enum ThreadMessage {
    Resize(Rect),
    Input(SshInput),
    NewTerm(CrosstermBackend<TerminalHandle>, ChannelId),
}

impl Display for ThreadMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadMessage::Resize(_) => write!(f, "Resize"),
            ThreadMessage::Input(_) => write!(f, "Input"),
            ThreadMessage::NewTerm(_, _) => write!(f, "New terminal"),
        }
    }
}

struct ClientTask {
    main_chanel: Option<ChannelId>,

    page: LoadedPage,

    rx: Receiver<ThreadMessage>,
    term: Option<SshTerminal>,
}

struct LoadedPage {
    animation: Option<Interval>,
    page: SshPage,
}

impl LoadedPage {
    async fn animation_interval(&mut self) -> Instant {
        if let Some(animation) = self.animation.as_mut() {
            animation.tick().await
        } else {
            pending::<Instant>().await
        }
    }
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

            match code {
                anyhow::Result::Ok(x) => match x {
                    Code::ChangeTo => {
                        self.page = self.page.page.slingshot().into();

                        let rendered = self.render().await;
                        self = rendered.ownership;
                        match rendered.results {
                            std::result::Result::Ok(_) => {}
                            std::result::Result::Err(RenderError::Panicked) => {
                                let _ = self.terminate().await;
                                return;
                            }
                            std::result::Result::Err(RenderError::InternalError(e)) => {
                                warn!("Encountered error doing rendering {e}");
                            }
                        }
                    }
                    Code::SkipRenderer => {
                        continue;
                    }
                    Code::Render => {
                        let rendered = self.render().await;
                        self = rendered.ownership;
                        match rendered.results {
                            std::result::Result::Ok(_) => {}
                            std::result::Result::Err(RenderError::Panicked) => {
                                let _ = self.terminate().await;
                                return;
                            }
                            std::result::Result::Err(RenderError::InternalError(e)) => {
                                warn!("Encountered error doing rendering {e}");
                            }
                        }
                    }
                    Code::Terminate => {
                        if let Err(error) = self.terminate().await {
                            warn!("Encountered error {error} while terminating connection")
                        };
                        return;
                    }
                },
                Err(err) => {
                    warn!("Error {err:?} reading data from task terminating",);
                    return;
                }
            }
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
