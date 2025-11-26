use std::{
    alloc::{Layout, alloc, dealloc},
    ptr::null_mut,
};

use chrono::{DateTime, Utc};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        self, Errno, uv_default_loop, uv_loop_alive, uv_loop_close, uv_loop_configure,
        uv_loop_init, uv_loop_option, uv_loop_set_data, uv_loop_t, uv_now, uv_run, uv_run_mode,
        uv_stop, uv_update_time,
    },
};

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq)]
pub enum Option {
    BLOCK_SIGNAL,
    METRICS_IDLE_TIME,
}

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq)]
pub enum RunMode {
    DEFAULT,
    ONCE,
    NOWAIT,
}

#[derive(Debug, Clone, Copy)]
pub struct Loop {
    raw: *mut uv_loop_t,
}

// TODO: impl uv_walk and get/set_context
impl Loop {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_loop_t>();
        let raw = unsafe { alloc(layout) as *mut uv_loop_t };
        super::init_loop(raw as *mut uv_loop_t);
        if raw.is_null() {
            return Err(Errno::ENOMEM);
        }

        let result = unsafe { uv_loop_init(raw) };
        if result < 0 {
            unsafe { dealloc(raw as *mut u8, layout) };
            return Err(Errno::from_inner(result));
        }
        Ok(Self { raw })
    }

    pub fn new_default() -> Result<Self, Errno> {
        let raw = unsafe { uv_default_loop() };
        if raw.is_null() {
            Err(Errno::ENOMEM)
        } else {
            Ok(Self { raw })
        }
    }

    pub fn configure(&mut self, option: Option) -> Result<(), Errno> {
        let result = unsafe { uv_loop_configure(self.raw, option.into_inner()) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    pub fn run(&mut self, mode: RunMode) -> Result<(), Errno> {
        let result = unsafe { uv_run(self.raw, mode.into_inner()) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    pub fn alive(&self) -> bool {
        unsafe { uv_loop_alive(self.raw) != 0 }
    }

    pub fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(unsafe { uv_now(self.raw) } as i64).unwrap()
    }

    pub fn update_time(&mut self) {
        unsafe { uv_update_time(self.raw) }
    }

    pub fn stop(&mut self) {
        unsafe { uv_stop(self.raw) }
    }

    // TODO: on close, walk handle list and close all open handles
    pub fn close(&mut self) -> Result<(), Errno> {
        let result = unsafe { uv_loop_close(self.raw) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }
}

pub(crate) fn init_loop(raw: *mut uv_loop_t) {
    unsafe { uv_loop_set_data(raw, null_mut()) };
}

impl Default for Loop {
    fn default() -> Self {
        Loop::new_default().unwrap()
    }
}

impl FromInner<*mut uv_loop_t> for Loop {
    fn from_inner(raw: *mut uv_loop_t) -> Self {
        Self { raw }
    }
}

impl FromInner<uv_loop_option> for Option {
    fn from_inner(value: uv_loop_option) -> Self {
        match value {
            uv::uv_loop_option_UV_LOOP_BLOCK_SIGNAL => Option::BLOCK_SIGNAL,
            uv::uv_loop_option_UV_METRICS_IDLE_TIME => Option::METRICS_IDLE_TIME,
            _ => unreachable!(),
        }
    }
}

impl FromInner<uv_run_mode> for RunMode {
    fn from_inner(value: uv_run_mode) -> Self {
        match value {
            uv::uv_run_mode_UV_RUN_DEFAULT => RunMode::DEFAULT,
            uv::uv_run_mode_UV_RUN_ONCE => RunMode::ONCE,
            uv::uv_run_mode_UV_RUN_NOWAIT => RunMode::NOWAIT,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<*mut uv_loop_t> for &Loop {
    fn into_inner(self) -> *mut uv_loop_t {
        self.raw
    }
}

impl IntoInner<*mut uv_loop_t> for &mut Loop {
    fn into_inner(self) -> *mut uv_loop_t {
        self.raw
    }
}

impl IntoInner<uv_loop_option> for Option {
    fn into_inner(self) -> uv_run_mode {
        match self {
            Option::BLOCK_SIGNAL => uv::uv_loop_option_UV_LOOP_BLOCK_SIGNAL,
            Option::METRICS_IDLE_TIME => uv::uv_loop_option_UV_METRICS_IDLE_TIME,
        }
    }
}

impl IntoInner<uv_run_mode> for RunMode {
    fn into_inner(self) -> uv_run_mode {
        match self {
            RunMode::DEFAULT => uv::uv_run_mode_UV_RUN_DEFAULT,
            RunMode::ONCE => uv::uv_run_mode_UV_RUN_ONCE,
            RunMode::NOWAIT => uv::uv_run_mode_UV_RUN_NOWAIT,
        }
    }
}
