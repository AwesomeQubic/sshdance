//This is a half serious I use for my ssh site so have fun :3

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::Ok;
use async_trait::async_trait;
use rand::prelude::IndexedRandom;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use sshdance::{
    site::{Code, Page, SshInput, SshPage},
    SshDanceBuilder,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2222);
    SshDanceBuilder::new(socket, |_| IntroPage::new())
        .run()
        .await
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

pub struct IntroPage {
    frame: usize,
    text: Vec<Line<'static>>,
}

impl IntroPage {
    pub fn new() -> SshPage {
        Box::new(IntroPage {
            frame: 0,
            text: Vec::with_capacity(100),
        }) as Box<(dyn Page + Send + Sync + 'static)>
    }
}

#[async_trait]
impl Page for IntroPage {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let start = self
            .text
            .len()
            .checked_sub(area.height as usize)
            .unwrap_or(0);
        let text = Paragraph::new(self.text[start..].to_vec());
        frame.render_widget(text, area);

        //Overlay render
        let cursor_color = {
            let step = self.frame / 10;
            if step % 2 == 0 {
                Color::LightMagenta
            } else {
                Color::Reset
            }
        };

        let line = Line::default()
            .spans([
                Span::styled(
                    "Qubic",
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
                "[Press enter to proceed]",
                Style::new().fg(Color::Reset).add_modifier(Modifier::BOLD),
            )])
            .centered();
        let new_area = Rect::new(area.x, centered_y + 1, area.width, 1);
        frame.render_widget(line, new_area);
    }

    fn get_tps(&self) -> Option<u16> {
        Some(10)
    }

    fn tick(&mut self) -> anyhow::Result<Code> {
        self.frame += 1;
        if self.text.len() == 100 {
            //TODO make faster
            self.text.remove(0);
        }
        self.text.push(Line::styled(
            *BOOT_SPLASH.choose(&mut rand::rng()).unwrap(),
            Style::new().fg(Color::DarkGray),
        ));
        Ok(Code::Render)
    }

    async fn handle_input(&mut self, _input: SshInput) -> anyhow::Result<Code> {
        Ok(Code::SkipRenderer)
    }
}
