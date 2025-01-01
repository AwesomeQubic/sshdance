//This is a half serious I use for my ssh site so have fun :3

use std::{
    f64::consts::E,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use anyhow::Ok;
use async_trait::async_trait;
use rand::{seq::SliceRandom, Rng};
use ratatui::{
    layout::{Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{
        canvas::{Canvas, Circle, Line, Rectangle},
        Paragraph,
    },
    Frame,
};
use sshdance::{
    site::{Code, Page, SshInput, SshPage},
    SshDanceBuilder,
};
use tracing::{info, warn};

const COLOGS: [Color; 4] = [Color::Green, Color::Red, Color::Magenta, Color::Cyan];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2222);
    SshDanceBuilder::new(socket, |_| Panic::new())
        .run()
        .await
}

pub struct Panic;

impl Panic {
    pub fn new() -> SshPage {
        Box::new(Panic) as SshPage
    }
}

#[async_trait]
impl Page for Panic {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        panic!("BOO");
    }

    async fn handle_input(&mut self, _input: SshInput) -> anyhow::Result<Code> {
        Ok(Code::Render)
    }
}
