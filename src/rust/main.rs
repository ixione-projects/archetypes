pub mod inners;
pub mod uv;

use std::{io::stdout, os::fd::AsRawFd};

use crate::uv::{
    Errno, HandleType, Loop, MutBuf, RunMode, WriteRequest, guess_handle,
    stream::{IStreamHandle, TTYMode},
};

fn main() -> Result<(), Errno> {
    let stdout = stdout().as_raw_fd();

    let mut uv_loop = Loop::default()?;
    let mut uv_tty = uv_loop.new_tty(stdout)?;
    uv_tty.set_mode(TTYMode::NORMAL)?;

    if guess_handle(stdout) == HandleType::TTY {
        let buf = MutBuf::from("\x1b[41;37m");
        let req = WriteRequest::new()?;
        uv_tty.write(&req, &[buf], ())?;
    }

    let buf = MutBuf::from("Hello TTY\n");
    let req = WriteRequest::new()?;
    uv_tty.write(&req, &[buf], |_, _| {
        println!("Goodbye TTY");
    })?;
    uv_tty.reset_mode()?;
    uv_loop.run(RunMode::DEFAULT)?;

    Ok(())
}
