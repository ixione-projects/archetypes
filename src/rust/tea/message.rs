use std::{error::Error, fmt::Debug};

use crate::tea::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    Terminate = 0,
    Interrupt = 1,
    Keypress = 2,
    Error = 3,
}

pub enum Message {
    Terminate,
    Interrupt,
    Keypress(KeyCode),
    Error(Box<dyn Error>),
}

impl Message {
    pub fn r#type(&self) -> MessageType {
        match self {
            Self::Terminate => MessageType::Terminate,
            Self::Interrupt => MessageType::Interrupt,
            Self::Keypress(_) => MessageType::Keypress,
            Self::Error(_) => MessageType::Error,
        }
    }
}

impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminate => write!(f, "Terminate"),
            Self::Interrupt => write!(f, "Interrupt"),
            Self::Keypress(keycode) => f.debug_tuple("Keypress").field(keycode).finish(),
            Self::Error(err) => f.debug_tuple("Error").field(err).finish(),
        }
    }
}

impl<T> From<T> for Message
where
    T: Error + 'static,
{
    fn from(err: T) -> Self {
        Message::Error(Box::new(err))
    }
}
