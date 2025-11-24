pub mod message;
pub use message::*;

pub mod model;
pub use model::*;

pub mod key;
pub use key::*;

pub mod error;
pub use error::*;

use std::{collections::HashMap, error::Error};

use crate::uv::{
    Buf, ConstBuf, Handle, HandleType, Loop, RunMode, buf, guess_handle,
    stream::{IStreamHandle, StreamHandle, TTYMode, TTYStream},
};

pub struct Position(usize, usize);

pub struct UpdateHandler<'a, M>(pub Box<dyn FnMut(&M, &Message) + 'a>);

pub struct UpdateBroker<'a, M> {
    handlers: HashMap<MessageType, Vec<UpdateHandler<'a, M>>>,
}

impl<'a, M> UpdateBroker<'a, M> {
    pub fn publish(&mut self, model: &M, msg: Message) -> bool {
        if let Some(handlers) = self.handlers.get_mut(&msg.r#type()) {
            for handler in handlers.iter_mut() {
                handler.0(&model, &msg);
            }
            true
        } else {
            false
        }
    }

    pub fn subscribe(&mut self, on: MessageType, handler: UpdateHandler<'a, M>) {
        match self.handlers.get_mut(&on) {
            Some(handlers) => handlers.push(handler.into()),
            None => {
                self.handlers.insert(on, vec![handler.into()]);
            }
        }
    }
}

impl<'a, M> Default for UpdateBroker<'a, M> {
    fn default() -> Self {
        Self {
            handlers: Default::default(),
        }
    }
}

// TODO: add interior mutability
pub struct Program<'a, M> {
    r#loop: Loop,
    r#in: TTYStream,
    r#out: TTYStream,

    width: i32,
    height: i32,
    cursor: Position,

    model: M,
    broker: UpdateBroker<'a, M>,
}

fn new_tty_stream(r#loop: Loop, fd: i32) -> Result<TTYStream, TUIError> {
    let guess = guess_handle(fd);
    if guess != HandleType::TTY {
        return Err(TUIError::InvalidHandleType(HandleType::TTY, guess));
    }
    Ok(r#loop.new_tty(fd)?)
}

impl<'a, M> Program<'a, M> {
    pub fn init(model: M, r#in: i32, r#out: i32) -> Result<Self, Box<dyn Error>> {
        let r#loop = Loop::default()?;
        let r#in = r#loop.new_tty(r#in)?;
        let r#out = r#loop.new_tty(r#out)?;
        let (width, height) = r#out.get_winsize()?;

        Ok(Self {
            r#loop,
            r#in,
            r#out,
            width: width.max(80),
            height: height.max(45),
            cursor: Position(0, 0),
            model,
            broker: Default::default(),
        })
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.r#in.set_mode(TTYMode::RAW)?;
        self.r#in.read_start(
            |_: &Handle, suggested_size| buf::new_with_capacity(suggested_size).ok(),
            |_: &StreamHandle, nread, buf: ConstBuf| {
                match nread {
                    Ok(len) => match buf.to_bytes(len as usize) {
                        Ok(bytes) => self
                            .broker
                            .publish(&self.model, Message::Keypress(bytes.to_owned())),
                        Err(err) => panic!("{}", err),
                    },
                    Err(err) => panic!("{}", err),
                };
            },
        )?;

        self.r#loop.run(RunMode::DEFAULT)?;
        Ok(self.r#in.set_mode(TTYMode::NORMAL)?)
    }

    pub fn on<UH>(&mut self, r#type: MessageType, handler: UH)
    where
        UH: Into<UpdateHandler<'a, M>>,
    {
        self.broker.subscribe(r#type, handler.into());
    }

    pub fn quit(&mut self) {
        self.r#in.read_stop();
    }
}

impl<'a, M, Fn> From<Fn> for UpdateHandler<'a, M>
where
    Fn: FnMut(&M, &Message) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}
