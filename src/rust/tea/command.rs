use crate::tea::message::Message;

pub type Command<'a> = &'a dyn FnMut() -> Message;

pub const TERMINATE: Command = &terminate;

pub fn terminate() -> Message {
    return Message::Terminate;
}
