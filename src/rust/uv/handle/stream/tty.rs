use std::{
    alloc::{Layout, alloc, dealloc},
    any::{Any, TypeId},
    os::raw::{c_int, c_void},
};

use crate::{
    inners::{FromInner, IntoInner},
    result,
    uv::{
        self, Errno, IHandle, IStreamHandle, Loop, StreamContext, uv_handle_t, uv_stream_t,
        uv_tty_get_vterm_state, uv_tty_get_winsize, uv_tty_init, uv_tty_mode_t, uv_tty_reset_mode,
        uv_tty_set_mode, uv_tty_set_vterm_state, uv_tty_t, uv_tty_vtermstate_t,
    },
};

// super

impl super::IStreamHandle for TTYStream {
    fn into_stream(self) -> super::StreamHandle {
        super::StreamHandle::from_inner(self.raw as *mut uv_stream_t)
    }

    fn drop_stream(self) {
        let layout = Layout::new::<uv_tty_t>();
        unsafe { dealloc(self.raw as *mut u8, layout) };
    }
}

impl super::IHandle for TTYStream {
    fn into_handle(self) -> uv::Handle {
        super::Handle::from_inner(self.raw as *mut uv_handle_t)
    }

    fn drop_handle(self) {
        self.drop_stream()
    }
}

// type

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    NORMAL,
    RAW,
    IO,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VTerminal {
    SUPPORTED,
    UNSUPPORTED,
}

#[derive(Debug, Clone, Copy)]
pub struct TTYStream {
    raw: *mut uv_tty_t,
}

// impl

impl TTYStream {
    fn new(r#loop: &Loop, fd: i32) -> Result<Self, Errno> {
        let layout = Layout::new::<uv_tty_t>();
        let raw = unsafe { alloc(layout) as *mut uv_tty_t };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        super::init_stream(raw as *mut uv_stream_t);

        let result = unsafe { uv_tty_init(r#loop.into_inner(), raw, fd, 0) };
        if result < 0 {
            unsafe { dealloc(raw as *mut u8, layout) };
            return Err(Errno::from_inner(result));
        }

        Ok(Self { raw })
    }

    pub fn set_mode(&mut self, mode: Mode) -> Result<(), Errno> {
        result!(unsafe { uv_tty_set_mode(self.raw, mode.into_inner()) })
    }

    pub fn reset_mode(&mut self) -> Result<(), Errno> {
        result!(unsafe { uv_tty_reset_mode() })
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

    pub fn set_vterminal_state(&mut self, state: VTerminal) {
        unsafe { uv_tty_set_vterm_state(state.into_inner()) }
    }

    pub fn get_vterminal_state(&mut self) -> Result<VTerminal, Errno> {
        let mut state: uv_tty_vtermstate_t = 0;
        let result = unsafe { uv_tty_get_vterm_state(&mut state) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(VTerminal::from_inner(state))
        }
    }

    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        let handle = self.into_handle();
        if let Some(context) = unsafe { handle.get_context::<StreamContext>() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "CheckHandle::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
                        TypeId::of::<D>(),
                        (*context.data).type_id()
                    ))
            })
        } else {
            None
        }
    }

    pub fn set_data<D: 'static>(&mut self, data: D) {
        let data = Box::into_raw(Box::new(data)) as *mut c_void;
        let mut handle = self.into_handle();
        match unsafe { handle.get_context::<StreamContext>() } {
            Some(context) => context.data = data,
            None => {
                handle.set_context(StreamContext {
                    alloc_cb: None,
                    close_cb: None,
                    data,
                    connection_cb: None,
                    read_cb: None,
                });
            }
        }
    }
}

impl Loop {
    pub fn new_tty(&self, fd: i32) -> Result<TTYStream, Errno> {
        return TTYStream::new(self, fd);
    }
}

// inner

impl FromInner<uv_tty_mode_t> for Mode {
    fn from_inner(value: uv_tty_mode_t) -> Self {
        match value {
            uv::uv_tty_mode_t_UV_TTY_MODE_NORMAL => Mode::NORMAL,
            uv::uv_tty_mode_t_UV_TTY_MODE_RAW => Mode::RAW,
            uv::uv_tty_mode_t_UV_TTY_MODE_IO => Mode::IO,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<uv_tty_mode_t> for Mode {
    fn into_inner(self) -> uv_tty_mode_t {
        match self {
            Mode::NORMAL => uv::uv_tty_mode_t_UV_TTY_MODE_NORMAL,
            Mode::RAW => uv::uv_tty_mode_t_UV_TTY_MODE_RAW,
            Mode::IO => uv::uv_tty_mode_t_UV_TTY_MODE_IO,
        }
    }
}

impl FromInner<uv_tty_vtermstate_t> for VTerminal {
    fn from_inner(value: uv_tty_vtermstate_t) -> Self {
        match value {
            uv::uv_tty_vtermstate_t_UV_TTY_SUPPORTED => VTerminal::SUPPORTED,
            uv::uv_tty_vtermstate_t_UV_TTY_UNSUPPORTED => VTerminal::UNSUPPORTED,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<uv_tty_vtermstate_t> for VTerminal {
    fn into_inner(self) -> uv_tty_vtermstate_t {
        match self {
            VTerminal::SUPPORTED => uv::uv_tty_vtermstate_t_UV_TTY_SUPPORTED,
            VTerminal::UNSUPPORTED => uv::uv_tty_vtermstate_t_UV_TTY_UNSUPPORTED,
        }
    }
}

impl FromInner<*mut uv_tty_t> for TTYStream {
    fn from_inner(raw: *mut uv_tty_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_tty_t> for TTYStream {
    fn into_inner(self) -> *mut uv_tty_t {
        self.raw
    }
}
