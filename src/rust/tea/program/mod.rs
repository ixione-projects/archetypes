pub mod update;
pub use update::*;

pub mod command;
pub use command::*;

pub mod error;
pub use error::*;

use std::{
    io::{stdin, stdout},
    os::fd::AsRawFd,
    str::from_utf8,
    sync::{Mutex, mpsc::channel},
    thread,
    time::Duration,
};

use crate::{
    tea::{KeyCodeParser, Message, MessageType, Model},
    uv::{
        Buf, CheckHandle, Errno, FileSystemRequest, Handle, HandleType, IHandle, IStreamHandle,
        Loop, Mode, RunMode, StreamHandle, TTYStream, WriteRequest, guess_handle,
    },
};

pub const CPR_REQUEST: &'static str = "\x1B[6n"; // TODO: change to \u

pub struct Program<'a, M: Model> {
    model: M,
    context: Mutex<ProgramContext>,
    r#loop: Loop,
    inner: Mutex<ProgramInner>,
    updates: UpdateBroker<'a, M>,
    keycode_parser: KeyCodeParser,
}

pub struct ProgramContext {
    pub width: i32,
    pub height: i32,
    pub home: (isize, isize),
    pub cursor: (isize, isize),
}

pub struct ProgramInner {
    r#in: TTYStream,
    r#out: TTYStream,
    messages: CheckHandle,
}

impl ProgramInner {
    pub fn terminate(&mut self) {
        self.r#in.read_stop();
        self.messages.stop();
    }
}

impl<'a, M: Model> Program<'a, M> {
    // TODO: init through an inner struct and impl drop to reset tty on init error
    pub fn init(model: M) -> Result<Self, ProgramError> {
        let r#loop = Loop::default();
        let stdin = stdin().as_raw_fd();
        let stdin_guess = guess_handle(stdin);
        if stdin_guess != HandleType::TTY {
            panic!("expected stdin to be TTY but found [{}]", stdin_guess);
        }

        let mut r#in = r#loop.new_tty(stdin)?;

        let stdout = stdout().as_raw_fd();
        let stdout_guess = guess_handle(stdin);
        if stdout_guess != HandleType::TTY {
            panic!("expected stdout to be TTY but found [{}]", stdout_guess);
        }

        let out = r#loop.new_tty(stdout)?;

        let result = try {
            let (width, height) = out.get_winsize()?;

            r#in.set_mode(Mode::RAW)?;
            r#loop.fs_write_sync(
                FileSystemRequest::new(),
                stdout,
                &[Buf::new(CPR_REQUEST)],
                -1,
            )?;

            // FIXME: unfortunately it looks like fs_write/fs_read will not work for tty
            let report = r#loop.fs_read_sync(
                FileSystemRequest::new(),
                stdin,
                &[Buf::new_with_capacity(32)],
                -1,
            )?;

            let home: (isize, isize);
            let mut keycode_parser = KeyCodeParser::default();
            let mut report_buf = Buf::join(report.0);
            keycode_parser.buffer(&report_buf.resize(report.1 as usize));
            match keycode_parser.parse_keycode() {
                Some(keycode) => {
                    let semi = keycode.code.iter().position(|ch| ch == &b';').unwrap();
                    let row = from_utf8(&keycode.code[2..semi]).unwrap().parse().unwrap();
                    let r = keycode.code.iter().position(|ch| ch == &b'R').unwrap();
                    let col = from_utf8(&keycode.code[semi + 1..r])
                        .unwrap()
                        .parse()
                        .unwrap();
                    home = (row, col);
                }
                None => {
                    return Err(ProgramError::InitError(format!(
                        "failed to parse cursor position report: {:?}",
                        report_buf.as_bytes()
                    )));
                }
            }

            let messages = r#loop.new_check()?;
            Self {
                model,
                context: Mutex::new(ProgramContext {
                    width,
                    height,
                    home,
                    cursor: home,
                }),
                r#loop,
                inner: Mutex::new(ProgramInner {
                    r#in,
                    out,
                    messages,
                }),
                updates: Default::default(),
                keycode_parser,
            }
        };

        if let Err(_) = result {
            r#in.set_mode(Mode::NORMAL)?;
        }

        Ok(result?)
    }

    // TODO: better error handling then just panic? Could we return some error command and shutdown gracefully?
    // NOTE: lock and send should not be recoverable
    pub fn run(&mut self) -> Result<(), ProgramError> {
        let (txmessage, rxmessage) = channel::<Message>();

        let txmessage_keypress = txmessage.clone();
        match self.inner.lock() {
            Ok(mut inner) => {
                inner.r#in.read_start(
                    |_: &Handle, suggested_size| Some(Buf::new_with_capacity(suggested_size)),
                    |_: &StreamHandle, nread, mut buf: Buf| {
                        match nread {
                            Ok(len) => {
                                self.keycode_parser.buffer(&buf.resize(len as usize));
                                while let Some(keycode) = self.keycode_parser.parse_keycode() {
                                    txmessage_keypress.send(Message::Keypress(keycode)).unwrap();
                                }
                            }
                            Err(err) => panic!("{}", err),
                        };
                    },
                )?;
            }
            Err(err) => panic!("{}", err),
        }

        let txmessage_command = txmessage.clone();
        match self.inner.lock() {
            Ok(mut inner) => {
                inner.messages.start(|handle: &CheckHandle| {
                    let mut terminating = false;
                    for message in rxmessage.try_iter() {
                        match message {
                            Message::Terminate => {
                                terminating = true;
                            }
                            _ => (),
                        }

                        self.updates.publish(
                            &mut self.model,
                            &self.context,
                            &self.inner,
                            handle.get_loop(),
                            &txmessage_command,
                            message,
                        );
                    }

                    if terminating {
                        self.inner.lock().unwrap().terminate();
                    }

                    match (self.context.lock(), self.inner.lock()) {
                        (Ok(context), Ok(mut inner)) => {
                            if let Err(err) = inner.out.write(
                                WriteRequest::new(),
                                &[
                                    Buf::new(format!(
                                        "\x1B[{};{}H",
                                        context.home.0, context.home.1,
                                    )),
                                    Buf::new(self.model.view()),
                                ],
                                (),
                            ) {
                                panic!("{}", err);
                            }
                        }
                        (Err(err), _) => panic!("{}", err),
                        (_, Err(err)) => panic!("{}", err),
                    }
                })?;
            }
            Err(err) => panic!("{}", err),
        }

        Ok(self.r#loop.run(RunMode::DEFAULT)?)
    }

    pub fn update<UH>(&mut self, r#type: MessageType, handler: UH)
    where
        UH: Into<UpdateHandler<'a, M>>,
    {
        self.updates.subscribe(r#type, handler.into());
    }
}

impl<'a, M: Model> Drop for Program<'a, M> {
    fn drop(&mut self) {
        self.inner.lock().unwrap().r#in.reset_mode().unwrap();
    }
}
