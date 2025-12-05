use std::{
    alloc::{Layout, alloc, dealloc},
    any::{Any, TypeId},
    os::raw::c_void,
    ptr::null_mut,
};

use chrono::{DateTime, Utc};

use crate::{
    inners::{FromInner, IntoInner},
    result,
    uv::{
        self, Errno, uv_default_loop, uv_loop_alive, uv_loop_close, uv_loop_configure,
        uv_loop_get_data, uv_loop_init, uv_loop_option, uv_loop_set_data, uv_loop_t, uv_now,
        uv_run, uv_run_mode, uv_stop, uv_update_time,
    },
};

// type

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationOption {
    BLOCK_SIGNAL,
    METRICS_IDLE_TIME,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    DEFAULT,
    ONCE,
    NOWAIT,
}

#[repr(C)]
pub struct LoopContext {
    data: *mut c_void,
}

#[derive(Debug, Clone, Copy)]
pub struct Loop {
    raw: *mut uv_loop_t,
    dealloc: bool,
}

// fn

pub(crate) fn init_loop(raw: *mut uv_loop_t) {
    unsafe { uv_loop_set_data(raw, null_mut()) };
}

// impl

// TODO: impl walk;
impl Loop {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_loop_t>();
        let raw = unsafe { alloc(layout) as *mut uv_loop_t };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        init_loop(raw);

        let result = unsafe { uv_loop_init(raw) };
        if result < 0 {
            unsafe { dealloc(raw as *mut u8, layout) };
            return Err(Errno::from_inner(result));
        }

        Ok(Self { raw, dealloc: true })
    }

    pub fn configure(&mut self, option: ConfigurationOption) -> Result<(), Errno> {
        result!(unsafe { uv_loop_configure(self.raw, option.into_inner()) })
    }

    pub fn close(mut self) -> Result<(), Errno> {
        let result = unsafe { uv_loop_close(self.raw) };
        if result < 0 {
            return Err(Errno::from_inner(result));
        }

        self.drop_context();
        if self.dealloc {
            let layout = Layout::new::<uv_loop_t>();
            unsafe { dealloc(self.raw as *mut u8, layout) }
        }

        Ok(())
    }

    pub fn run(&mut self, mode: RunMode) -> Result<(), Errno> {
        result!(unsafe { uv_run(self.raw, mode.into_inner()) })
    }

    pub fn alive(&self) -> bool {
        unsafe { uv_loop_alive(self.raw) != 0 }
    }

    pub fn stop(&mut self) {
        unsafe { uv_stop(self.raw) }
    }

    pub fn now(&self) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp_millis(unsafe { uv_now(self.raw) } as i64)
    }

    pub fn update_time(&mut self) {
        unsafe { uv_update_time(self.raw) }
    }

    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        if let Some(context) = unsafe { self.get_context() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "Loop::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
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
        match unsafe { self.get_context() } {
            Some(context) => context.data = data,
            None => {
                self.set_context(LoopContext { data });
            }
        }
    }

    pub(crate) unsafe fn get_context(&self) -> Option<&mut LoopContext> {
        let context = uv_loop_get_data(self.raw);
        if context.is_null() {
            None
        } else {
            Some(&mut *(context as *mut LoopContext))
        }
    }

    pub(crate) fn set_context(&mut self, context: LoopContext) {
        unsafe { uv_loop_set_data(self.raw, Box::into_raw(Box::new(context)) as *mut c_void) };
    }

    pub(crate) fn drop_context(&mut self) {
        if let Some(context) = unsafe { self.get_context() } {
            if !context.data.is_null() {
                drop(unsafe { Box::from_raw(context.data) })
            }
            drop(unsafe { Box::from_raw(context) })
        }
    }
}

// trait

impl Default for Loop {
    fn default() -> Self {
        let raw = unsafe { uv_default_loop() };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }
        Self {
            raw,
            dealloc: false,
        }
    }
}

// inner

impl FromInner<uv_loop_option> for ConfigurationOption {
    fn from_inner(value: uv_loop_option) -> Self {
        match value {
            uv::uv_loop_option_UV_LOOP_BLOCK_SIGNAL => ConfigurationOption::BLOCK_SIGNAL,
            uv::uv_loop_option_UV_METRICS_IDLE_TIME => ConfigurationOption::METRICS_IDLE_TIME,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<uv_loop_option> for ConfigurationOption {
    fn into_inner(self) -> uv_run_mode {
        match self {
            ConfigurationOption::BLOCK_SIGNAL => uv::uv_loop_option_UV_LOOP_BLOCK_SIGNAL,
            ConfigurationOption::METRICS_IDLE_TIME => uv::uv_loop_option_UV_METRICS_IDLE_TIME,
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

impl IntoInner<uv_run_mode> for RunMode {
    fn into_inner(self) -> uv_run_mode {
        match self {
            RunMode::DEFAULT => uv::uv_run_mode_UV_RUN_DEFAULT,
            RunMode::ONCE => uv::uv_run_mode_UV_RUN_ONCE,
            RunMode::NOWAIT => uv::uv_run_mode_UV_RUN_NOWAIT,
        }
    }
}

impl FromInner<*mut uv_loop_t> for Loop {
    fn from_inner(raw: *mut uv_loop_t) -> Self {
        Self {
            raw,
            dealloc: false,
        }
    }
}

impl IntoInner<*mut uv_loop_t> for Loop {
    fn into_inner(self) -> *mut uv_loop_t {
        self.raw
    }
}
