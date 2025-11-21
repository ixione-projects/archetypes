use std::{
    alloc::{Layout, alloc, dealloc},
    os::raw::c_int,
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        self, Errno, Loop, uv_stream_t, uv_tty_get_winsize, uv_tty_init, uv_tty_mode_t,
        uv_tty_reset_mode, uv_tty_set_mode, uv_tty_t,
    },
};

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum TTYMode {
    NORMAL,
    RAW,
    IO,
}

impl super::IStreamHandle for TTYStream {
    fn into_stream(self) -> super::StreamHandle {
        super::StreamHandle::from_raw(self.raw as *mut uv_stream_t)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TTYStream {
    raw: *mut uv_tty_t,
}

impl TTYStream {
    fn new(r#loop: &Loop, fd: i32) -> Result<Self, Errno> {
        let layout = Layout::new::<uv_tty_t>();
        let raw = unsafe { alloc(layout) as *mut uv_tty_t };
        super::init_stream(raw as *mut uv_stream_t);
        if raw.is_null() {
            return Err(Errno::ENOMEM);
        }

        let result = unsafe { uv_tty_init(r#loop.into_inner(), raw, fd, 0) };
        if result < 0 {
            unsafe { dealloc(raw as *mut u8, layout) };
            return Err(Errno::from_inner(result));
        }
        Ok(Self { raw })
    }

    pub fn set_mode(&mut self, mode: TTYMode) -> Result<(), Errno> {
        let result = unsafe { uv_tty_set_mode(self.raw, mode.into_inner()) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    pub fn reset_mode(&mut self) -> Result<(), Errno> {
        let result = unsafe { uv_tty_reset_mode() };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    pub fn get_winsize(&self) -> Result<(i32, i32), Errno> {
        let mut width: c_int = 0;
        let mut height: c_int = 0;
        let result = unsafe { uv_tty_get_winsize(self.raw, &mut width, &mut height) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok((width, height))
        }
    }
}

impl Loop {
    pub fn new_tty(&self, fd: i32) -> Result<TTYStream, Errno> {
        return TTYStream::new(self, fd);
    }
}

impl FromInner<uv_tty_mode_t> for TTYMode {
    fn from_inner(value: uv_tty_mode_t) -> Self {
        match value {
            uv::uv_tty_mode_t_UV_TTY_MODE_NORMAL => TTYMode::NORMAL,
            uv::uv_tty_mode_t_UV_TTY_MODE_RAW => TTYMode::RAW,
            uv::uv_tty_mode_t_UV_TTY_MODE_IO => TTYMode::IO,
            _ => unreachable!(),
        }
    }
}

impl FromInner<*mut uv_tty_t> for TTYStream {
    fn from_inner(raw: *mut uv_tty_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<uv_tty_mode_t> for TTYMode {
    fn into_inner(self) -> uv_tty_mode_t {
        match self {
            TTYMode::NORMAL => uv::uv_tty_mode_t_UV_TTY_MODE_NORMAL,
            TTYMode::RAW => uv::uv_tty_mode_t_UV_TTY_MODE_RAW,
            TTYMode::IO => uv::uv_tty_mode_t_UV_TTY_MODE_IO,
        }
    }
}

impl IntoInner<*mut uv_tty_t> for &mut TTYStream {
    fn into_inner(self) -> *mut uv_tty_t {
        self.raw
    }
}
