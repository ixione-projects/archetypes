use std::{
    collections::HashMap,
    sync::{Mutex, mpsc::Sender},
};

use crate::{
    tea::{Command, Message, MessageType, Model, ProgramContext, ProgramInner},
    uv::{Loop, WorkRequest},
};

pub struct UpdateHandler<'a, M: Model>(
    pub Box<dyn FnMut(&mut M, &ProgramContext, &Message) -> Option<Box<dyn Command<M>>> + 'a>,
);

pub struct UpdateBroker<'a, M: Model> {
    handlers: HashMap<MessageType, UpdateHandler<'a, M>>,
}

impl<'a, M: Model> UpdateBroker<'a, M> {
    pub fn publish(
        &mut self,
        model: &mut M,
        context: &Mutex<ProgramContext>,
        inner: &Mutex<ProgramInner>,
        r#loop: Loop,
        txmessage: &Sender<Message>,
        msg: Message,
    ) {
        if let Some(handler) = self.handlers.get_mut(&msg.r#type()) {
            match context.lock() {
                Ok(acquired_context) => {
                    if let Some(mut cmd) = handler.0(model, &acquired_context, &msg) {
                        if let Err(err) = r#loop.queue_work(
                            WorkRequest::new(),
                            |_| {
                                let message = cmd.call(context, inner);
                                if let Message::Terminate = message {
                                    self.publish(model, context, inner, r#loop, txmessage, message);
                                } else {
                                    txmessage.send(message).unwrap();
                                }
                            },
                            (),
                        ) {
                            txmessage.send(Message::from(err)).unwrap();
                        }
                    }
                }
                Err(err) => panic!("{}", err),
            }
        }
    }

    pub fn subscribe(&mut self, on: MessageType, handler: UpdateHandler<'a, M>) {
        self.handlers.insert(on, handler);
    }
}

impl<'a, M: Model> Default for UpdateBroker<'a, M> {
    fn default() -> Self {
        Self {
            handlers: Default::default(),
        }
    }
}

impl<'a, Fn, M: Model> From<Fn> for UpdateHandler<'a, M>
where
    Fn: FnMut(&mut M, &ProgramContext, &Message) -> Option<Box<dyn Command<M>>> + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}
