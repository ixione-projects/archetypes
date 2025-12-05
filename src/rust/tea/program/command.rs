use std::sync::Mutex;

use crate::tea::{ProgramContext, ProgramInner, message::Message, model::Model};

pub trait Command<M: Model> {
    fn call(&mut self, context: &Mutex<ProgramContext>, inner: &Mutex<ProgramInner>) -> Message;
}

pub struct Terminate;
impl<M: Model> Command<M> for Terminate {
    fn call(&mut self, _: &Mutex<ProgramContext>, _: &Mutex<ProgramInner>) -> Message {
        Message::Terminate
    }
}

impl<M: Model> From<Terminate> for Box<dyn Command<M>> {
    fn from(value: Terminate) -> Self {
        Box::new(value)
    }
}
