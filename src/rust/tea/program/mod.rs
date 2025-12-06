pub mod update;
pub use update::*;

pub mod command;
pub use command::*;

pub mod error;
pub use error::*;

use std::{
    cell::RefCell,
    io::{stdin, stdout},
    os::fd::AsRawFd,
    rc::Rc,
    str::from_utf8,
    sync::{Mutex, mpsc::channel},
};

use crate::{
    tea::{KeyCodeParser, Message, MessageType, Model},
    uv::{
        Buf, CheckHandle, ConvertBuf, Handle, HandleType, IHandle, IStreamHandle, Loop, Mode,
        RunMode, StreamHandle, TTYStream, WriteRequest, guess_handle,
    },
};

pub const CPR_REQUEST: &'static str = "\u{1b}[6n";

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
    pub fn init(model: M) -> Result<Self, ProgramError> {
        struct InitDropGaurd {
            r#in: RefCell<TTYStream>,
        }
        impl Drop for InitDropGaurd {
            fn drop(&mut self) {
                self.r#in.borrow_mut().set_mode(Mode::NORMAL).unwrap();
            }
        }

        let mut r#loop = Loop::default();

        let stdin = stdin().as_raw_fd();
        let stdin_guess = guess_handle(stdin);
        if stdin_guess != HandleType::TTY {
            panic!("expected stdin to be TTY but found [{}]", stdin_guess);
        }

        let r#in: Rc<TTYStream> = Rc::new(r#loop.new_tty(stdin)?);
        let guard = InitDropGaurd {
            r#in: RefCell::new(*r#in.clone()),
        };

        let stdout = stdout().as_raw_fd();
        let stdout_guess = guess_handle(stdin);
        if stdout_guess != HandleType::TTY {
            panic!("expected stdout to be TTY but found [{}]", stdout_guess);
        }

        let mut out = r#loop.new_tty(stdout)?;
        let (width, height) = out.get_winsize()?;

        guard.r#in.borrow_mut().set_mode(Mode::RAW)?;

        let mut report = Buf::new();
        guard.r#in.borrow_mut().read_start(
            |_: &Handle, suggested_size| Some(Buf::new_with_len(suggested_size)),
            |_: &StreamHandle, nread, buf: Buf| {
                match nread {
                    Ok(len) => {
                        report.append(&buf.as_ref()[..len as usize].to_buf());
                        if report[report.len() - 2] == b'R' {
                            guard.r#in.borrow_mut().read_stop();
                        }
                    }
                    Err(err) => panic!("{}", err),
                };
            },
        )?;

        out.write(WriteRequest::new(), &[Buf::from(CPR_REQUEST)], ())?;

        r#loop.run(RunMode::DEFAULT)?;

        let mut keycode_parser = KeyCodeParser::default();
        keycode_parser.buffer(&report);
        let home = match keycode_parser.parse_keycode() {
            Some(keycode) => {
                let semi = keycode.code.iter().position(|ch| ch == &b';').unwrap();
                let row = from_utf8(&keycode.code[2..semi])?.parse()?;
                let r = keycode.code.iter().position(|ch| ch == &b'R').unwrap();
                let col = from_utf8(&keycode.code[semi + 1..r])?.parse()?;

                Ok((row, col))
            }
            None => Err(ProgramError::InitError(format!(
                "failed to parse cursor position report: {:?}",
                report.as_bytes()
            ))),
        }?;

        let messages = r#loop.new_check()?;
        Ok(Self {
            model,
            context: Mutex::new(ProgramContext {
                width,
                height,
                home,
                cursor: home,
            }),
            r#loop,
            inner: Mutex::new(ProgramInner {
                r#in: *r#in,
                out,
                messages,
            }),
            updates: Default::default(),
            keycode_parser,
        })
    }

    pub fn run(&mut self) -> Result<(), ProgramError> {
        self.inner.lock().unwrap().r#in.set_mode(Mode::RAW)?;

        let (txmessage, rxmessage) = channel::<Message>();

        let txmessage_keypress = txmessage.clone();
        match self.inner.lock() {
            Ok(mut inner) => {
                inner.r#in.read_start(
                    |_: &Handle, suggested_size| Some(Buf::new_with_len(suggested_size)),
                    |_: &StreamHandle, nread, buf: Buf| {
                        match nread {
                            Ok(len) => {
                                self.keycode_parser
                                    .buffer(&buf.as_ref()[..len as usize].to_buf());
                                while let Some(keycode) = self.keycode_parser.parse_keycode() {
                                    txmessage_keypress.send(Message::Keypress(keycode)).unwrap();
                                }
                            }
                            Err(err) => {
                                txmessage_keypress.send(Message::from(err)).unwrap();
                            }
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
                    for message in rxmessage.try_iter() {
                        self.updates.publish(
                            &mut self.model,
                            &self.context,
                            &self.inner,
                            handle.get_loop(),
                            &txmessage_command,
                            message,
                        );
                    }

                    match (self.context.lock(), self.inner.lock()) {
                        (Ok(context), Ok(mut inner)) => {
                            if let Err(err) = inner.out.write(
                                WriteRequest::new(),
                                &[
                                    Buf::from(format!(
                                        "\x1B[{};{}H",
                                        context.home.0, context.home.1,
                                    )),
                                    Buf::from(self.model.view()),
                                ],
                                (),
                            ) {
                                txmessage_command.send(Message::from(err)).unwrap();
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
