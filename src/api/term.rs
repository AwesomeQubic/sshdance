use std::num::NonZero;

use ratatui::{
    layout::Rect,
    Frame,
};
use termwiz::input::{InputEvent, KeyCode, Modifiers};
use tokio::sync::mpsc::UnboundedSender;

#[allow(unused_variables)]
pub trait SshTerminal: Sized + Sync + Send + 'static {
    type MessageType: Send;
    const DEFAULT_TPS: Option<NonZero<u8>> = None;

    fn on_input(&mut self, engine: &mut impl EngineRef<Self>, input: InputEvent) -> CallbackRez {
        if let InputEvent::Key(key_event) = input {
            if key_event.key == KeyCode::Char('d') && key_event.modifiers.contains(Modifiers::CTRL)
            {
                return CallbackRez::Terminate("See you next time\nSmelly furries".to_string());
            }
        }
        CallbackRez::PushToRenderer
    }

    fn on_resize(
        &mut self,
        engine: &mut impl EngineRef<Self>,
        width: u16,
        height: u16,
    ) -> CallbackRez {
        CallbackRez::PushToRenderer
    }

    fn on_message(
        &mut self,
        engine: &mut impl EngineRef<Self>,
        message: Self::MessageType,
    ) -> CallbackRez {
        CallbackRez::Continue
    }

    fn on_animation(&mut self, engine: &mut impl EngineRef<Self>) -> CallbackRez {
        CallbackRez::Continue
    }

    fn draw(&mut self, frame: &mut Frame<'_>);
}

pub trait EngineRef<T: SshTerminal> {
    fn terminal_channel(&mut self) -> UnboundedSender<T::MessageType>;

    fn current_size(&mut self) -> Rect;
}

pub enum CallbackRez {
    PushToRenderer,
    Continue,
    Terminate(String),
}

impl CallbackRez {
    pub(crate) fn pick(self, other: CallbackRez) -> CallbackRez {
        match self {
            CallbackRez::PushToRenderer => match other {
                CallbackRez::Continue => CallbackRez::PushToRenderer,
                x => x,
            },
            CallbackRez::Continue => other,
            CallbackRez::Terminate(x) => CallbackRez::Terminate(x),
        }
    }
}
