#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    Keypress = 0,
}

#[derive(Debug, Clone)]
pub enum Message {
    Keypress(Vec<u8>),
}

impl Message {
    pub fn r#type(&self) -> MessageType {
        match self {
            Message::Keypress(_) => MessageType::Keypress,
        }
    }
}
