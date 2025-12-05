use std::fmt::Debug;

use crate::tea::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    Terminate = 0,
    Interrupt = 1,
    Keypress = 2,
}

pub enum Message {
    Terminate,
    Interrupt,
    Keypress(KeyCode),
}

impl Message {
    pub fn r#type(&self) -> MessageType {
        match self {
            Message::Terminate => MessageType::Terminate,
            Message::Interrupt => MessageType::Interrupt,
            Message::Keypress(_) => MessageType::Keypress,
        }
    }
}

impl Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminate => write!(f, "Terminate"),
            Self::Interrupt => write!(f, "Interrupt"),
            Self::Keypress(arg0) => f.debug_tuple("Keypress").field(arg0).finish(),
        }
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Self::Terminate => Self::Terminate,
            Self::Interrupt => Self::Interrupt,
            Self::Keypress(arg0) => Self::Keypress(arg0.clone()),
        }
    }
}
