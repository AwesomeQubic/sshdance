use std::future::pending;
use std::io::Write;
use std::mem::replace;

use crate::site::Code;
use crate::site::EscapeCode;
use crate::site::SshInput;
use crate::site::SshPage;
use crate::util::SshTerminal;
use crate::util::TerminalHandle;
use anyhow::Context;
use anyhow::Ok;
use anyhow::Result;
use async_trait::async_trait;
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
use tracing::debug;
use tracing::info;
use tracing::trace;
use tracing::warn;

pub struct ClientHandler {
    thread: JoinHandle<()>,
    tx: Sender<ThreadMessage>,
}

impl Drop for ClientHandler {
    fn drop(&mut self) {
        info!("Closing connection...");
        self.thread.abort();
    }
}

impl ClientHandler {
    pub fn new(ip: Option<std::net::SocketAddr>, page: SshPage) -> ClientHandler {
        let (tx, rx) = mpsc::channel::<ThreadMessage>(100);
        let task = ClientTask {
            rx,
            term: None,
            main_chanel: None,
            ip,
            page: page.into(),
        };
        let thread = tokio::task::spawn(task.run());

        ClientHandler { thread, tx }
    }
}

#[async_trait]
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

        let backend = CrosstermBackend::new(terminal_handle);

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

struct ClientTask {
    main_chanel: Option<ChannelId>,

    ip: Option<std::net::SocketAddr>,

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

impl Drop for ClientTask {
    fn drop(&mut self) {
        debug!("Droping client task");
    }
}

impl ClientTask {
    async fn run(mut self) {
        loop {
            let rx_future = self.rx.recv();
            let anim_future = self.page.animation_interval();
            tokio::select! {
                message = rx_future => {
                    let Some(event) = message else {
                        return;
                    };

                    match self.handle_input(event).await {
                        anyhow::Result::Ok(x) => match x {
                            Code::ChangeTo(x) => {
                                self.page = x.into();
                                if let Err(error) = self.render().await {
                                    warn!("Frame rendering failed with error {error} after changing to new page");
                                }
                            }
                            Code::SkipRenderer => {
                                continue;
                            }
                            Code::Render => {
                                if let Err(error) = self.render().await {
                                    warn!("Frame rendering failed with error {error}");
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
                            warn!(
                                "{:?}: Error {err:?} reading data from task terminating",
                                self.ip
                            );
                            return;
                        }
                    }
                },
                _anim = anim_future => {
                    let code = self.page.page.tick();
                    match code {
                        anyhow::Result::Ok(x) => match x {
                            Code::ChangeTo(x) => {
                                self.page = x.into();
                                if let Err(error) = self.render().await {
                                    warn!("Encountered error while rendering page {error} after a page switch");
                                }
                            }
                            Code::SkipRenderer => {
                                continue;
                            }
                            Code::Render => {
                                if let Err(error) = self.render().await {
                                    warn!("Encountered error while rendering page {error}");
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
                            warn!(
                                "{:?}: Error {err:?} reading data from task terminating",
                                self.ip
                            );
                            return;
                        }
                    }
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

            let writer = backend.writer_mut();

            writer.flush()?;
            writer.close().await?;
        }
        Ok(())
    }

    async fn handle_input(&mut self, message: ThreadMessage) -> Result<Code> {
        match message {
            ThreadMessage::NewTerm(mut backend, id) => {
                backend.execute(EnterAlternateScreen)?;
                //backend.execute(EnableMouseCapture)?;
                backend.execute(cursor::Hide)?;
                backend.execute(terminal::SetTitle("You are connected to SITE NAME"))?;

                info!("{:?}: Creating term", self.ip);

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
                info!("{:?}: Resizing term {rect}", self.ip);
                self.term
                    .as_mut()
                    .context("No term initialized")?
                    .resize(rect)?;
                Ok(Code::Render)
            }
            ThreadMessage::Input(SshInput::Special(EscapeCode::CtrlC)) => {
                Ok(Code::Terminate)
            }
            ThreadMessage::Input(x) => return self.page.page.handle_input(x).await,
        }
    }

    async fn render(&mut self) -> Result<(), anyhow::Error> {
        debug!("{:?}: Redrawing terminal", self.ip);
        let term = self.term.as_mut().context("No term")?;
        let renderer = &mut *self.page.page;

        term.draw(|frame| {
            let area = frame.area();
            renderer.render(frame, area);
        })
        .unwrap();

        Ok(())
    }
}
