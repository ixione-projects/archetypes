use std::alloc::{Layout, alloc, dealloc};

use crate::uv::{
    self, Errno, Loop,
    stream::{self, Stream, StreamHandle},
    uv_errno_t, uv_stream_t, uv_tty_init, uv_tty_mode_t, uv_tty_reset_mode, uv_tty_set_mode,
    uv_tty_t,
};

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum TTYMode {
    NORMAL,
    RAW,
    IO,
}

impl From<uv_tty_mode_t> for TTYMode {
    fn from(value: uv_tty_mode_t) -> Self {
        match value {
            uv::uv_tty_mode_t_UV_TTY_MODE_NORMAL => TTYMode::NORMAL,
            uv::uv_tty_mode_t_UV_TTY_MODE_RAW => TTYMode::RAW,
            uv::uv_tty_mode_t_UV_TTY_MODE_IO => TTYMode::IO,
            _ => unreachable!(),
        }
    }
}

impl Into<uv_tty_mode_t> for TTYMode {
    fn into(self) -> uv_tty_mode_t {
        match self {
            TTYMode::NORMAL => uv::uv_tty_mode_t_UV_TTY_MODE_NORMAL,
            TTYMode::RAW => uv::uv_tty_mode_t_UV_TTY_MODE_RAW,
            TTYMode::IO => uv::uv_tty_mode_t_UV_TTY_MODE_IO,
        }
    }
}

pub struct TTYStream {
    raw: *mut uv_tty_t,
}

impl From<*mut uv_tty_t> for TTYStream {
    fn from(raw: *mut uv_tty_t) -> Self {
        Self { raw }
    }
}

impl Into<*mut uv_tty_t> for &mut TTYStream {
    fn into(self) -> *mut uv_tty_t {
        self.raw
    }
}

impl TTYStream {
    fn new(r#loop: &Loop, fd: i32) -> Result<Self, Errno> {
        let layout = Layout::new::<uv_tty_t>();
        let raw = unsafe { alloc(layout) as *mut uv_tty_t };
        if raw.is_null() {
            return Err(Errno::ENOMEM);
        }

        let result = unsafe { uv_tty_init(r#loop.into(), raw, fd, 0) };
        if result < 0 {
            unsafe { dealloc(raw as *mut u8, layout) };
            return Err(Errno::from(result as uv_errno_t));
        }
        Ok(Self { raw })
    }

    pub fn set_mode(&mut self, mode: TTYMode) -> Result<(), Errno> {
        let result = unsafe { uv_tty_set_mode(self.into(), mode.into()) };
        if result < 0 {
            Err(Errno::from(result as uv_errno_t))
        } else {
            Ok(())
        }
    }

    pub fn reset_mode(&mut self) -> Result<(), Errno> {
        let result = unsafe { uv_tty_reset_mode() };
        if result < 0 {
            Err(Errno::from(result as uv_errno_t))
        } else {
            Ok(())
        }
    }
}

impl StreamHandle for TTYStream {
    fn into_stream(&self) -> stream::Stream {
        Stream::from_raw(self.raw as *mut uv_stream_t)
    }
}

impl Loop {
    pub fn new_tty(&self, fd: i32) -> Result<TTYStream, Errno> {
        return TTYStream::new(self, fd);
    }
}
