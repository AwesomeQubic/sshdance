//This is a half serious I use for my ssh site so have fun :3

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
    Frame,
};
use sshdance::{
    api::{
        term::SshTerminal,
        utils::SimpleTerminalHandler,
    },
    SshDanceBuilder,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

#[tokio::main]
async fn main() -> Result<(), sshdance::Error> {
    tracing_subscriber::fmt::init();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2223);
    SshDanceBuilder::<SimpleTerminalHandler<HelloWorld>>::new(socket)
        .run()
        .await
        .unwrap();
    Ok(())
}

#[derive(Default)]
pub struct HelloWorld {}

impl SshTerminal for HelloWorld {
    type MessageType = ();

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let line = ratatui::text::Line::default()
            .spans([Span::styled(
                "Hello ssh",
                Style::new().add_modifier(Modifier::BOLD).fg(Color::Reset),
            )])
            .centered();
        frame.render_widget(line, frame.area());
    }
}
