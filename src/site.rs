use std::os::unix::net::SocketAddr;

use anyhow::Ok;
use async_trait::async_trait;
use ratatui::layout::Rect;
use ratatui::Frame;

pub type DrawFunc = dyn for<'a> FnOnce(&mut Frame<'a>) + Send + Sync;

pub trait PageHandler: Sync + Send {
    fn create(ip: Option<std::net::SocketAddr>) -> Self;

    async fn setup(&mut self, engine: &mut impl EngineRef);

    async fn handle_raw_input(&mut self, input: &[u8], engine: &mut impl EngineRef) {}

    async fn handle_input(&mut self, input: SshInput, engine: &mut impl EngineRef);

    async fn animate(&mut self, engine: &mut impl EngineRef);

    async fn resize(&mut self, engine: &mut impl EngineRef);

    async fn notify(&mut self, engine: &mut impl EngineRef);
}

pub trait EngineRef {
    fn set_tick_rate(&mut self, tick_rate: Option<u8>);

    fn terminate<'a, T: Into<&'a str>>(&mut self, message: T);

    fn set_window_title<'a, T: Into<&'a str>>(&mut self, message: T);

    fn render<'a, T: FnOnce(&mut Frame<'a>) + Send + Sync>(func: DrawFunc);
}

pub enum SshInput {
    KeyPress(char),
    Special(EscapeCode),
}

pub enum EscapeCode {
    Esc,
    Enter,
    Up,
    Down,
    Right,
    Left,
    CtrlC,
}
