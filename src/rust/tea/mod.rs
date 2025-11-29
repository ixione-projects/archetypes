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
    sync::{
        Arc, Mutex,
        mpsc::{Sender, channel},
    },
};

use crate::{
    tea::{
        command::Command,
        error::TEAError,
        message::{Message, MessageType},
        model::Model,
    },
    uv::{
        Buf, ConstBuf, Handle, HandleType, IHandle, Loop, RunMode, WorkRequest, WriteRequest, buf,
        check::CheckHandle,
        guess_handle,
        stream::{IStreamHandle, StreamHandle, TTYMode, TTYStream},
    },
};

pub struct UpdateHandler<'a, M: Model>(
    pub Box<dyn FnMut(&ProgramContext<M>, &Message) -> Option<Box<dyn Command<M>>> + 'a>,
);

pub struct UpdateBroker<'a, M: Model> {
    handlers: HashMap<MessageType, UpdateHandler<'a, M>>,
}

pub struct ProgramContext<M: Model> {
    pub width: i32,
    pub height: i32,
    pub cursor: (usize, usize),
    pub model: RefCell<M>,
}

pub struct ProgramInner {
    r#loop: Loop,
    r#in: TTYStream,
    r#out: TTYStream,
    r#messages: CheckHandle,
}

pub struct Program<'a, M: Model> {
    context: Arc<Mutex<ProgramContext<M>>>,
    inner: Arc<Mutex<ProgramInner>>,
    updates: RefCell<UpdateBroker<'a, M>>,
}

fn new_tty_stream(r#loop: Loop, fd: i32) -> Result<TTYStream, TEAError> {
    let guess = guess_handle(fd);
    if guess != HandleType::TTY {
        return Err(TEAError::InvalidHandleType(HandleType::TTY, guess));
    }
    Ok(r#loop.new_tty(fd)?)
}

impl<'a, M: Model> UpdateBroker<'a, M> {
    pub fn publish(
        &mut self,
        context: Arc<Mutex<ProgramContext<M>>>,
        inner: Arc<Mutex<ProgramInner>>,
        r#loop: Loop,
        txmessage: &Sender<Message>,
        msg: Message,
    ) {
        if let Some(handler) = self.handlers.get_mut(&msg.r#type()) {
            if let Some(mut cmd) = handler.0(&context.lock().unwrap(), &msg) {
                let req = WorkRequest::new().unwrap();
                let work_context = context.clone();
                let inner_context = inner.clone();
                r#loop.queue_work(
                    req,
                    |_| {
                        txmessage.send(cmd.call(&work_context, &inner_context));
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

impl<'a, M: Model> Program<'a, M> {
    pub fn init(model: M) -> Result<Self, Box<dyn Error>> {
        let r#loop = Loop::new_default()?;
        let r#in = new_tty_stream(r#loop, stdin().as_raw_fd())?;
        let out = new_tty_stream(r#loop, stdout().as_raw_fd())?;
        let messages = r#loop.new_check()?;
        let (width, height) = out.get_winsize()?;

        Ok(Self {
            context: Arc::new(Mutex::new(ProgramContext {
                width: width.max(80),
                height: height.max(45),
                cursor: (0, 0),
                model: RefCell::new(model),
            })),
            inner: Arc::new(Mutex::new(ProgramInner {
                r#loop,
                r#in,
                r#out,
                r#messages,
            })),
            updates: Default::default(),
        })
    }

    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let (txmessage, rxmessage) = channel::<Message>();

        let mut read_start_handle = self.inner.lock().unwrap().r#in.clone();
        let mut read_stop_handle = self.inner.lock().unwrap().r#in.clone();
        let txmessage_keypress = txmessage.clone();
        read_start_handle.read_start(
            |_: &Handle, suggested_size| buf::new_with_capacity(suggested_size).ok(),
            |_: &StreamHandle, nread, buf: ConstBuf| {
                // TODO: handle err
                let _ = match nread {
                    Ok(len) => match buf.to_bytes(len as usize) {
                        Ok(bytes) => txmessage_keypress.send(Message::Keypress(bytes.to_owned())),
                        Err(err) => panic!("{}", err),
                    },
                    Err(err) => panic!("{}", err),
                };
            },
        )?;

        let mut messages_start_handle = self.inner.lock().unwrap().messages.clone();
        let mut messages_stop_handle = self.inner.lock().unwrap().messages.clone();
        let mut messages_write = self.inner.lock().unwrap().out.clone();
        let txmessage_command = txmessage.clone();
        messages_start_handle.start(|handle: &CheckHandle| {
            let mut terminating = false;
            for message in rxmessage.try_iter() {
                if let Message::Terminate = message {
                    terminating = true;
                }

                self.updates.borrow_mut().publish(
                    self.context.clone(),
                    self.inner.clone(),
                    handle.get_loop(),
                    &txmessage_command,
                    message,
                );
            }

            if terminating {
                read_stop_handle.read_stop();
                messages_stop_handle.stop();
            }

            let req = WriteRequest::new().unwrap();
            messages_write.write(
                req,
                &[
                    ConstBuf::from(self.context.lock().unwrap().model.borrow().view()),
                    ConstBuf::from("\r"),
                ],
                (),
            );
        })?;

        self.inner.lock().unwrap().r#in.set_mode(TTYMode::RAW)?;
        self.inner.lock().unwrap().r#loop.run(RunMode::DEFAULT)?;
        Ok(self.inner.lock().unwrap().r#in.set_mode(TTYMode::NORMAL)?)
    }

    pub fn update<UH>(&self, r#type: MessageType, handler: UH)
    where
        UH: Into<UpdateHandler<'a, M>>,
    {
        self.updates.borrow_mut().subscribe(r#type, handler.into());
    }
}

impl Clone for ProgramInner {
    fn clone(&self) -> Self {
        Self {
            r#loop: self.r#loop.clone(),
            r#in: self.r#in.clone(),
            r#out: self.r#out.clone(),
            r#messages: self.r#messages.clone(),
        }
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
    Fn: FnMut(&ProgramContext<M>, &Message) -> Option<Box<dyn Command<M>>> + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}
