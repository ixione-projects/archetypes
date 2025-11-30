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
    str::from_utf8,
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
        Buf, ConstBuf, FileSystemRequest, Handle, HandleType, IHandle, Loop, MutBuf, RunMode,
        WorkRequest, WriteRequest, buf,
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
    pub home: (isize, isize),
    pub cursor: (isize, isize),
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
                r#loop
                    .queue_work(
                        req,
                        |_| {
                            txmessage
                                .send(cmd.call(&work_context, &inner_context))
                                .unwrap();
                        },
                        (),
                    )
                    .unwrap();
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
        let stdin = stdin().as_raw_fd();
        let stdout = stdout().as_raw_fd();
        let r#in = new_tty_stream(r#loop, stdin)?;
        let out = new_tty_stream(r#loop, stdout)?;
        let messages = r#loop.new_check()?;
        let (width, height) = out.get_winsize()?;

        let program = Self {
            context: Arc::new(Mutex::new(ProgramContext {
                width: width.max(80),
                height: height.max(45),
                home: (0, 0),
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
        };

        // TODO: clean-up
        program.inner.lock().unwrap().r#in.set_mode(TTYMode::RAW)?;
        let read_handle = program.inner.lock().unwrap().r#loop.clone();
        let report: MutBuf = buf::new_with_capacity(32).unwrap();
        if let Err(err) = program.inner.lock().unwrap().r#loop.fs_write(
            FileSystemRequest::new().unwrap(),
            stdout,
            &[ConstBuf::from("\x1B[6n")],
            -1,
            |_: FileSystemRequest| {
                if let Err(err) = read_handle.fs_read(
                    FileSystemRequest::new().unwrap(),
                    stdin,
                    &[report],
                    -1,
                    |read_req: FileSystemRequest| {
                        if read_req.result() != 7 {
                            panic!("get_tty_home: invalid nread [{:?}]", read_req.result());
                        }

                        let context_ref = program.context.clone();
                        let mut context = context_ref.lock().unwrap();
                        let (mut s, mut c) = (2, 2);
                        while report[c].is_ascii_digit() {
                            c += 1;
                        }
                        context.home.0 = from_utf8(&report[s..c]).unwrap().parse().unwrap();
                        context.cursor.0 = from_utf8(&report[s..c]).unwrap().parse().unwrap();
                        (s, c) = (c + 1, c + 1);
                        while report[c].is_ascii_digit() {
                            c += 1;
                        }
                        context.home.1 = from_utf8(&report[s..c]).unwrap().parse().unwrap();
                        context.cursor.1 = from_utf8(&report[s..c]).unwrap().parse().unwrap();
                    },
                ) {
                    panic!("{:?}", TEAError::InternalUVError(err));
                }
            },
        ) {
            panic!("{:?}", TEAError::InternalUVError(err));
        };

        program.inner.lock().unwrap().r#loop.run(RunMode::DEFAULT)?;
        program
            .inner
            .lock()
            .unwrap()
            .r#in
            .set_mode(TTYMode::NORMAL)?;

        Ok(program)
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

            let context = self.context.lock().unwrap();
            messages_write
                .write(
                    WriteRequest::new().unwrap(),
                    &[
                        ConstBuf::from(format!("\x1B[{};{}H", context.home.0, context.home.1)),
                        ConstBuf::from(context.model.borrow().view()),
                    ],
                    (),
                )
                .unwrap();
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
