use crate::{
    tea::{ProgramContext, ProgramInner, message::Message},
    uv::stream::IStreamHandle,
};

pub trait Command<M> {
    fn call(&mut self, context: &mut ProgramContext<M>, inner: &mut ProgramInner) -> Message;
}

pub struct Terminate;
impl<M> Command<M> for Terminate {
    fn call(&mut self, context: &mut ProgramContext<M>, inner: &mut ProgramInner) -> Message {
        inner.r#in.read_stop();

        context.terminating = true;
        Message::Terminate
    }
}

impl<M> From<Terminate> for Box<dyn Command<M>> {
    fn from(value: Terminate) -> Self {
        Box::new(value)
    }
}
