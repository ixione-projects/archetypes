pub mod command;
pub mod message;

pub mod model;
pub use model::*;

pub mod key;
pub use key::*;

pub mod error;
pub use error::*;

use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    io::{stdin, stdout},
    os::fd::AsRawFd,
};

use crate::{
    tea::message::{Message, MessageType},
    uv::{
        Buf, ConstBuf, Handle, HandleType, IHandle, Loop, RunMode, buf, guess_handle,
        stream::{IStreamHandle, StreamHandle, TTYMode, TTYStream},
    },
};

pub struct UpdateHandler<'a, M: Clone>(
    pub Box<dyn FnMut(&ProgramContext<M>, &Message) -> Option<Message> + 'a>,
);

pub struct UpdateBroker<'a, M: Clone> {
    handlers: HashMap<MessageType, UpdateHandler<'a, M>>,
}

pub struct ProgramContext<'a, M: Clone> {
    width: i32,
    height: i32,
    cursor: (usize, usize),
    model: M,

    quit: &'a mut dyn FnMut(),
}

pub struct Program<'a, M: Clone> {
    width: i32,
    height: i32,
    cursor: (usize, usize),

    r#loop: RefCell<Loop>,
    r#in: RefCell<TTYStream>,
    r#out: RefCell<TTYStream>,

    model: M,
    updates: RefCell<UpdateBroker<'a, M>>,
}

fn new_tty_stream(r#loop: Loop, fd: i32) -> Result<TTYStream, TEAError> {
    let guess = guess_handle(fd);
    if guess != HandleType::TTY {
        return Err(TEAError::InvalidHandleType(HandleType::TTY, guess));
    }
    Ok(r#loop.new_tty(fd)?)
}

impl<'a, M: Clone> UpdateBroker<'a, M> {
    pub fn publish(&mut self, context: ProgramContext<M>, _: Loop, msg: Message) {
        if let Some(handler) = self.handlers.get_mut(&msg.r#type()) {
            if let Some(_) = handler.0(&context, &msg) {
                (context.quit)();
            }
        }
    }

    pub fn subscribe(&mut self, on: MessageType, handler: UpdateHandler<'a, M>) {
        self.handlers.insert(on, handler);
    }
}

impl<'a, M: Clone> Program<'a, M> {
    pub fn init(model: M) -> Result<Self, Box<dyn Error>> {
        let r#loop = Loop::new_default()?;
        let r#in = new_tty_stream(r#loop, stdin().as_raw_fd())?;
        let r#out = new_tty_stream(r#loop, stdout().as_raw_fd())?;
        let (width, height) = r#out.get_winsize()?;

        Ok(Self {
            width: width.max(80),
            height: height.max(45),
            cursor: (0, 0),
            r#loop: RefCell::new(r#loop),
            r#in: RefCell::new(r#in),
            r#out: RefCell::new(out),
            model,
            updates: Default::default(),
        })
    }

    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        self.r#in.borrow_mut().set_mode(TTYMode::RAW)?;
        self.r#in.borrow_mut().read_start(
            |_: &Handle, suggested_size| buf::new_with_capacity(suggested_size).ok(),
            |stream: &StreamHandle, nread, buf: ConstBuf| {
                match nread {
                    Ok(len) => match buf.to_bytes(len as usize) {
                        Ok(bytes) => {
                            self.updates.borrow_mut().publish(
                                ProgramContext {
                                    width: self.width,
                                    height: self.height,
                                    cursor: self.cursor,
                                    model: self.model.clone(),
                                    quit: &mut || {
                                        stream.into_stream().read_stop(); // copy
                                    },
                                },
                                stream.get_loop(),
                                Message::Keypress(bytes.to_owned()),
                            );
                        }
                        Err(err) => panic!("{}", err),
                    },
                    Err(err) => panic!("{}", err),
                };
            },
        )?;

        self.r#loop.borrow_mut().run(RunMode::DEFAULT)?;
        Ok(self.r#in.borrow_mut().set_mode(TTYMode::NORMAL)?)
    }

    pub fn on<UH>(&self, r#type: MessageType, handler: UH)
    where
        UH: Into<UpdateHandler<'a, M>>,
    {
        self.updates.borrow_mut().subscribe(r#type, handler.into());
    }
}

impl<'a, M: Clone> Default for UpdateBroker<'a, M> {
    fn default() -> Self {
        Self {
            handlers: Default::default(),
        }
    }
}

impl<'a, Fn, M: Clone> From<Fn> for UpdateHandler<'a, M>
where
    Fn: FnMut(&ProgramContext<M>, &Message) -> Option<Message> + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}
