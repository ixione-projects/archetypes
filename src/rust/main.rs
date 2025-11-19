pub mod uv;

use std::{io::stdout, os::fd::AsRawFd};

use crate::uv::{
    Buf, Errno, HandleType, Loop, RunMode, guess_handle,
    stream::{StreamHandle, TTYMode, WriteRequest},
};

fn main() -> Result<(), Errno> {
    let mut uv_loop = Loop::default()?;
    let mut uv_tty = uv_loop.new_tty(stdout().as_raw_fd())?;
    uv_tty.set_mode(TTYMode::NORMAL)?;

    if guess_handle(stdout().as_raw_fd()) == HandleType::TTY {
        let buf = Buf::from("\x1b[41;37m");
        let req = WriteRequest::new()?;
        uv_tty.write(&req, &[buf])?;
    }

    let buf = Buf::from("Hello TTY\n");
    let req = WriteRequest::new()?;
    uv_tty.write(&req, &[buf])?;
    uv_tty.reset_mode()?;
    uv_loop.run(RunMode::DEFAULT)?;

    Ok(())
}
