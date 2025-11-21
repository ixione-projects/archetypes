pub mod inners;
pub mod uv;

use std::{
    error::Error,
    fmt::Display,
    io::{stdin, stdout},
    os::fd::AsRawFd,
};

use crate::uv::{
    Buf, ConstBuf, Errno, Handle, HandleType, Loop, RunMode, WriteRequest, buf, guess_handle,
    stream::{IStreamHandle, StreamHandle, TTYMode},
};

#[derive(Debug)]
pub enum CLIError {
    Usage(&'static str),
    Internal(Errno),
}

impl Display for CLIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for CLIError {}

const USAGE: &str = "Usage: ./archetypes";

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = stdin().as_raw_fd();
    let stdout = stdout().as_raw_fd();
    if !ensure_tty(stdin, stdout) {
        return Err(Box::new(CLIError::Usage(USAGE)));
    }

    let mut r#loop = Loop::default()?;
    let mut reader = r#loop.new_tty(stdout)?;
    let mut writer = r#loop.new_tty(stdout)?;

    let mut saw_ctrl_c = false;

    reader.set_mode(TTYMode::RAW)?;
    if let Err(err) = reader.read_start(
        |_: &Handle<ConstBuf>, suggested_size| buf::new_with_capacity(suggested_size).ok(),
        move |stream: &StreamHandle, nread: Result<isize, _>, buf: ConstBuf| match nread {
            Ok(len) => {
                if let Ok(str) = buf.to_str(len as usize) {
                    if saw_ctrl_c && str.as_bytes()[0] == 03 {
                        stream.into_stream().read_stop();
                    } else if !saw_ctrl_c && str.as_bytes()[0] == 03 {
                        let req = WriteRequest::new().unwrap();
                        writer.write(
                            req,
                            &[ConstBuf::from("\n(Press ctrl-c again to exit.)\n")],
                            (),
                        );
                        saw_ctrl_c = true;
                    } else {
                        if str.as_bytes()[0] == 13 {
                            let req = WriteRequest::new().unwrap();
                            writer.write(req, &[ConstBuf::from("\n")], ());
                        } else {
                            let req = WriteRequest::new().unwrap();
                            writer.write(req, &[ConstBuf::from(str)], ());
                        }
                        saw_ctrl_c = false;
                    }
                }
            }
            Err(err) => {
                eprintln!("{}", err);
                stream.into_stream().read_stop();
            }
        },
    ) {
        eprintln!("{}", err);
        reader.read_stop();
    }

    r#loop.run(RunMode::DEFAULT)?;
    while r#loop.alive() {
        r#loop.run(RunMode::DEFAULT)?;
    }

    reader.set_mode(TTYMode::NORMAL)?;
    Ok(())
}

fn ensure_tty(stdin: i32, stdout: i32) -> bool {
    let (stdin_guess, stdout_guess) = (guess_handle(stdin), guess_handle(stdout));
    if stdin_guess != HandleType::TTY || stdout_guess != HandleType::TTY {
        eprintln!(
            "expected a tty but found: stdin({:?}) and stdout({:?})",
            stdin_guess, stdout_guess
        );
        false
    } else {
        true
    }
}
