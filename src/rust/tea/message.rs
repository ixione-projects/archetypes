#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    Terminate = 0,
    Interrupt = 1,
    Keypress = 2,
}

#[derive(Debug, Clone)]
pub enum Message {
    Terminate,
    Interrupt,
    Keypress(Vec<u8>),
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
