use std::{iter, net::{IpAddr, Ipv4Addr, SocketAddr}};

use anyhow::Ok;
use async_trait::async_trait;
use crossterm::style::Colors;
use rand::seq::SliceRandom;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use sshdance::{site::{Code, Page, SshInput, SshPage}, SshDanceBuilder};

#[tokio::main]
async fn main() {
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2222);
    SshDanceBuilder::new(socket, |_| {IntroPage::new()}).run().await;
}

const BOOT_SPLASH: &[&'static str] = &[
    "[ OK ] Checking if meme driven development is real",
    "[ OK ] Petting Java",
    "[ OK ] Making the ThePrimeagen cry after I steal his terminal shop intro",
    "[ OK ] Processing hate mail",
    "[ OK ] Processing memes",
    "[ OK ] Making unsolicited furry art",
    "[ OK ] Making your mum jokes",
    "[ OK ] Awaiting DMCA from terminal.shop",
];

pub struct IntroPage {
    frame: usize,
    text: Vec<Line<'static>>
}

impl IntroPage {
    pub fn new() -> SshPage {
        Box::new(IntroPage { frame: 0, text: Vec::with_capacity(100) })
    }
}

#[async_trait]
impl Page for IntroPage {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let start = self.text.len().checked_sub(area.height as usize).unwrap_or(0);
        let slice = &self.text[start..];
        let text = Paragraph::new(slice.to_vec()).style(Style::new().fg(Color::DarkGray));
        frame.render_widget(text, area);

        //Overlay render
        let cursor_color = {
            let step = self.frame / 4;
            if step % 2 == 0 {
                Color::LightMagenta
            } else {
                Color::Reset
            }
        };

        let line = Line::default().spans([
            Span::styled("Qubic", Style::new().bold().fg(Color::Reset)),
            Span::styled(" ", Style::new().bg(cursor_color)),
        ]).centered();
        let centered_y = area.height / 2;
        let new_area = Rect::new(area.x, centered_y, area.width, 1);
        frame.render_widget(line, new_area);
    }

    fn get_tps(&self) -> Option<u16> {
        Some(4)
    }

    async fn tick(&mut self) -> anyhow::Result<Code> {
        self.frame += 1;
        if self.text.len() == 100 {
            //TODO make faster
            self.text.remove(0);
        }
        self.text.push(Line::raw(*BOOT_SPLASH.choose(&mut rand::thread_rng()).unwrap()));
        Ok(Code::Render)
    }

    async fn handle_input(&mut self, _input: SshInput) -> anyhow::Result<Code> {
        Ok(Code::SkipRenderer)
    }
}
