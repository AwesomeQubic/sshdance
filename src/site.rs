use anyhow::Ok;
use async_trait::async_trait;
use ratatui::layout::Rect;
use ratatui::Frame;

pub type SshPage = Box<dyn Page + Sync + Send>;

#[async_trait]
pub trait Page {
    async fn handle_input(&mut self, input: SshInput) -> anyhow::Result<Code>;

    fn tick(&mut self) -> anyhow::Result<Code> {
        Ok(Code::SkipRenderer)
    }

    fn render(&mut self, frame: &mut Frame<'_>, rect: Rect);

    fn update_screen_rect(&mut self, rect: Rect) {}

    fn get_tps(&self) -> Option<u16> {
        None
    }
}

pub enum Code {
    ChangeTo(SshPage),
    SkipRenderer,
    Render,
    Terminate,
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
