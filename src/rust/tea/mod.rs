pub mod message;
pub use message::*;

pub mod model;
pub use model::*;

pub mod key;
pub use key::*;

pub mod error;
pub use error::*;

use std::{cell::RefCell, collections::HashMap, error::Error};

use crate::uv::{
    Buf, ConstBuf, Handle, HandleType, Loop, RunMode, buf, guess_handle,
    stream::{IStreamHandle, StreamHandle, TTYMode, TTYStream},
};

pub struct UpdateHandler<'a, M>(pub Box<dyn FnMut(&M, &Message) -> Option<Command> + 'a>);

pub struct UpdateBroker<'a, M> {
    handlers: HashMap<MessageType, Vec<UpdateHandler<'a, M>>>,
}

struct Inner {
    r#loop: Loop,
    r#in: TTYStream,
    r#out: TTYStream,
}

pub struct Position(usize, usize);

pub struct Program<'a, M> {
    width: i32,
    height: i32,
    cursor: Position,

    model: M,
    inner: RefCell<Inner>,
    updates: RefCell<UpdateBroker<'a, M>>,
}

fn new_tty_stream(r#loop: Loop, fd: i32) -> Result<TTYStream, TUIError> {
    let guess = guess_handle(fd);
    if guess != HandleType::TTY {
        return Err(TUIError::InvalidHandleType(HandleType::TTY, guess));
    }
    Ok(r#loop.new_tty(fd)?)
}

impl<'a, M> UpdateBroker<'a, M> {
    pub fn publish(&mut self, model: &M, msg: Message) -> Vec<Command> {
        let mut cmds = Vec::new();
        if let Some(handlers) = self.handlers.get_mut(&msg.r#type()) {
            for handler in handlers.iter_mut() {
                if let Some(cmd) = handler.0(&model, &msg) {
                    cmds.push(cmd);
                }
            }
        }
        cmds
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

impl<'a, M> Program<'a, M> {
    pub fn init(model: M, r#in: i32, r#out: i32) -> Result<Self, Box<dyn Error>> {
        let r#loop = Loop::new_default()?;
        let r#in = new_tty_stream(r#loop, r#in)?;
        let r#out = new_tty_stream(r#loop, r#out)?;
        let (width, height) = r#out.get_winsize()?;

        Ok(Self {
            width: width.max(80),
            height: height.max(45),
            cursor: Position(0, 0),
            model,
            inner: RefCell::new(Inner {
                r#loop,
                r#in,
                r#out,
            }),
            updates: Default::default(),
        })
    }

    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        self.inner.borrow_mut().r#in.set_mode(TTYMode::RAW)?;
        self.inner.borrow_mut().r#in.read_start(
            |_: &Handle, suggested_size| buf::new_with_capacity(suggested_size).ok(),
            |stream: &StreamHandle, nread, buf: ConstBuf| {
                match nread {
                    Ok(len) => match buf.to_bytes(len as usize) {
                        Ok(bytes) => {
                            for cmd in self
                                .updates
                                .borrow_mut()
                                .publish(&self.model, Message::Keypress(bytes.to_owned()))
                            {
                                match cmd {
                                    Command::Quit => stream.into_stream().read_stop(),
                                }
                            }
                        }
                        Err(err) => panic!("{}", err),
                    },
                    Err(err) => panic!("{}", err),
                };
            },
        )?;

        self.inner.borrow_mut().r#loop.run(RunMode::DEFAULT)?;
        Ok(self.inner.borrow_mut().r#in.set_mode(TTYMode::NORMAL)?)
    }

    pub fn on<UH>(&self, r#type: MessageType, handler: UH)
    where
        UH: Into<UpdateHandler<'a, M>>,
    {
        self.updates.borrow_mut().subscribe(r#type, handler.into());
    }
}

impl<'a, M> Default for UpdateBroker<'a, M> {
    fn default() -> Self {
        Self {
            handlers: Default::default(),
        }
    }
}

impl<'a, M, Fn> From<Fn> for UpdateHandler<'a, M>
where
    Fn: FnMut(&M, &Message) -> Option<Command> + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}
