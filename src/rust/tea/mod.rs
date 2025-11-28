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
    sync::mpsc::{Sender, channel},
};

use crate::{
    tea::{
        command::Command,
        error::TEAError,
        message::{Message, MessageType},
    },
    uv::{
        Buf, ConstBuf, Handle, HandleType, IHandle, Loop, RunMode, WorkRequest, buf,
        check::CheckHandle,
        guess_handle,
        stream::{IStreamHandle, StreamHandle, TTYMode, TTYStream},
    },
};

pub struct UpdateHandler<'a, M>(
    pub Box<dyn FnMut(&ProgramContext<M>, &Message) -> Option<Box<dyn Command<M>>> + 'a>,
);

pub struct UpdateBroker<'a, M> {
    handlers: HashMap<MessageType, UpdateHandler<'a, M>>,
}

#[derive(Debug, Clone)]
pub struct ProgramContext<M> {
    pub terminating: bool,
    pub width: i32,
    pub height: i32,
    pub cursor: (usize, usize),
    pub model: RefCell<M>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProgramInner {
    r#loop: Loop,
    r#in: TTYStream,
    r#out: TTYStream,
    r#messages: CheckHandle,
}

pub struct Program<'a, M> {
    context: RefCell<ProgramContext<M>>,
    inner: RefCell<ProgramInner>,
    updates: RefCell<UpdateBroker<'a, M>>,
}

fn new_tty_stream(r#loop: Loop, fd: i32) -> Result<TTYStream, TEAError> {
    let guess = guess_handle(fd);
    if guess != HandleType::TTY {
        return Err(TEAError::InvalidHandleType(HandleType::TTY, guess));
    }
    Ok(r#loop.new_tty(fd)?)
}

impl<'a, M> UpdateBroker<'a, M> {
    pub fn publish(
        &mut self,
        context: &mut ProgramContext<M>,
        inner: &mut ProgramInner,
        r#loop: Loop,
        txmessage: &Sender<Message>,
        msg: Message,
    ) {
        if let Some(handler) = self.handlers.get_mut(&msg.r#type()) {
            if let Some(mut cmd) = handler.0(context, &msg) {
                let req = WorkRequest::new().unwrap();
                r#loop.queue_work(
                    req,
                    |_| {
                        txmessage.send(cmd.call(context, inner));
                    },
                    (),
                );
            }
        }
    }

    pub fn subscribe(&mut self, on: MessageType, handler: UpdateHandler<'a, M>) {
        self.handlers.insert(on, handler);
    }
}

impl<'a, M> Program<'a, M> {
    pub fn init(model: M) -> Result<Self, Box<dyn Error>> {
        let r#loop = Loop::new_default()?;
        let r#in = new_tty_stream(r#loop, stdin().as_raw_fd())?;
        let out = new_tty_stream(r#loop, stdout().as_raw_fd())?;
        let messages = r#loop.new_check()?;
        let (width, height) = out.get_winsize()?;

        Ok(Self {
            context: RefCell::new(ProgramContext {
                terminating: false,
                width: width.max(80),
                height: height.max(45),
                cursor: (0, 0),
                model: RefCell::new(model),
            }),
            inner: RefCell::new(ProgramInner {
                r#loop,
                r#in,
                r#out,
                r#messages,
            }),
            updates: Default::default(),
        })
    }

    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let (txmessage, rxmessage) = channel::<Message>();

        let mut read_start_handle = self.inner.borrow_mut().r#in.clone();
        let txmessage_keypress = txmessage.clone();
        read_start_handle.read_start(
            |_: &Handle, suggested_size| buf::new_with_capacity(suggested_size).ok(),
            move |_: &StreamHandle, nread, buf: ConstBuf| {
                let _ = match nread {
                    Ok(len) => match buf.to_bytes(len as usize) {
                        Ok(bytes) => txmessage_keypress.send(Message::Keypress(bytes.to_owned())),
                        Err(err) => panic!("{}", err),
                    },
                    Err(err) => panic!("{}", err),
                };
            },
        )?;

        let mut messages_start_handle = self.inner.borrow_mut().messages.clone();
        let mut messages_stop_handle = self.inner.borrow_mut().messages.clone();
        let txmessage_command = txmessage.clone();
        let mut messages_program_inner = self.inner.borrow_mut().clone();
        messages_start_handle.start(move |handle: &CheckHandle| {
            for message in rxmessage.try_iter() {
                self.updates.borrow_mut().publish(
                    &mut self.context.borrow_mut(),
                    &mut messages_program_inner,
                    handle.get_loop(),
                    &txmessage_command,
                    message,
                );
            }

            if self.context.borrow().terminating {
                messages_stop_handle.stop();
            }
        })?;

        self.inner.borrow_mut().r#in.set_mode(TTYMode::RAW)?;
        self.inner.borrow_mut().r#loop.run(RunMode::DEFAULT)?;
        Ok(self.inner.borrow_mut().r#in.set_mode(TTYMode::NORMAL)?)
    }

    pub fn update<UH>(&self, r#type: MessageType, handler: UH)
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

impl<'a, Fn, M> From<Fn> for UpdateHandler<'a, M>
where
    Fn: FnMut(&ProgramContext<M>, &Message) -> Option<Box<dyn Command<M>>> + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}
