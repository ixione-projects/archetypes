pub mod command;
pub mod error;
pub mod key;
pub mod message;
pub mod model;

use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    io::{stdin, stdout},
    os::fd::AsRawFd,
    sync::mpsc::channel,
};

use crate::{
    tea::{
        error::TEAError,
        message::{Message, MessageType},
    },
    uv::{
        Buf, ConstBuf, Handle, HandleType, IHandle, Loop, RunMode, buf,
        check::CheckHandle,
        guess_handle,
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
        let mut messages_check = self.r#loop.borrow_mut().new_check()?;

        let mut messages_check_stop_handle = messages_check.clone();
        let mut read_stop_handle = self.r#in.borrow_mut().clone();

        let (txmessage, rxmessage) = channel::<Message>();

        let txmessage_keypress = txmessage.clone();
        self.r#in.borrow_mut().read_start(
            |_: &Handle, suggested_size| buf::new_with_capacity(suggested_size).ok(),
            move |_: &StreamHandle, nread, buf: ConstBuf| {
                match nread {
                    Ok(len) => match buf.to_bytes(len as usize) {
                        Ok(bytes) => txmessage_keypress.send(Message::Keypress(bytes.to_owned())),
                        Err(err) => panic!("{}", err),
                    },
                    Err(err) => panic!("{}", err),
                };
            },
        )?;

        messages_check.start(move |handle: &CheckHandle| {
            for received in rxmessage.try_iter() {
                self.updates.borrow_mut().publish(
                    ProgramContext {
                        width: self.width,
                        height: self.height,
                        cursor: self.cursor,
                        model: self.model.clone(),
                        quit: &mut || {
                            read_stop_handle.read_stop();
                            messages_check_stop_handle.stop();
                        },
                    },
                    handle.get_loop(),
                    received,
                );
            }
        })?;

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
