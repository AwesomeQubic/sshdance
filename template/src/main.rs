use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZero,
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use sshdance::{
    api::{
        term::{CallbackRez, SshTerminal},
        utils::SimpleTerminalHandler,
    },
    SshDanceBuilder,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2222);
    SshDanceBuilder::<SimpleTerminalHandler<IntroTerminal>>::new(socket)
        .run()
        .await
        .unwrap();
}

const BOOT_SPLASH: &[&'static str] = &[
    "[ DONE ] Checking if meme driven development is real",
    "[ DONE ] Petting Java",
    "[ DONE ] Making the ThePrimeagen cry after I steal his terminal shop intro",
    "[ DONE ] Processing hate mail",
    "[ DONE ] Processing memes",
    "[ DONE ] Making unsolicited furry art",
    "[ DONE ] Making your mum jokes",
    "[ DONE ] Awaiting DMCA from terminal.shop",
    "[ DONE ] Reading furry hate",
    "[ DONE ] Crying over people calling me cringe",
    "[ DONE ] Not using go for my project",
    "[ DONE ] Using 100% safe rust for my blazingly fast terminal server",
    "[ DONE ] Exfiltrating your credit card data",
    "[ DONE ] Spoofing some US election votes",
    "[ DONE ] Calculating π",
    "[ DONE ] Encrypting your hard drive for Ransomware",
    "[ DONE ] Mining some crypto",
    "[ DONE ] Running Crysis",
    "[ ERR ] Solving world hunger",
    "[ DONE ] Installing malware",
];

const fn get_splash(index: usize) -> &'static str {
    BOOT_SPLASH[index % BOOT_SPLASH.len()]
}

#[derive(Default)]
struct IntroTerminal {
    phase: usize,
}

impl SshTerminal for IntroTerminal {
    type MessageType = ();
    const DEFAULT_TPS: Option<std::num::NonZero<u8>> = Some(NonZero::new(5).unwrap());

    fn on_animation(
        &mut self,
        _engine: &mut impl sshdance::api::term::EngineRef<Self>,
    ) -> CallbackRez {
        self.phase += 1;
        CallbackRez::PushToRenderer
    }

    fn draw(&mut self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        let max = (area.height as usize).max(self.phase);
        for i in 0..max {
            let line = Line::from(get_splash(self.phase + i));
            frame.render_widget(
                line,
                Rect {
                    x: 0,
                    y: (i as u16),
                    width: area.width,
                    height: 1,
                },
            );
        }

        //Overlay render
        let cursor_color = {
            let step = self.phase / 10;
            if step % 2 == 0 {
                Color::LightMagenta
            } else {
                Color::Reset
            }
        };

        let line = Line::default()
            .spans([
                Span::styled(
                    "SSHDance",
                    Style::new().add_modifier(Modifier::BOLD).fg(Color::Reset),
                ),
                Span::styled(" ", Style::new().bg(cursor_color)),
            ])
            .centered();
        let centered_y = area.height / 2;
        let new_area = Rect::new(area.x, centered_y, area.width, 1);
        frame.render_widget(line, new_area);

        let line = Line::default()
            .spans([Span::styled(
                "[Install from crates to proceed]",
                Style::new().fg(Color::Reset).add_modifier(Modifier::BOLD),
            )])
            .centered();
        let new_area = Rect::new(area.x, centered_y + 1, area.width, 1);
        frame.render_widget(line, new_area);
    }
}
