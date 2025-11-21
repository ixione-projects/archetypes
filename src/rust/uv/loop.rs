use crate::{
    inners::{FromInner, IntoInner},
    uv::{self, Errno, uv_default_loop, uv_loop_alive, uv_loop_t, uv_run, uv_run_mode},
};

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum RunMode {
    DEFAULT,
    ONCE,
    NOWAIT,
}

pub struct Loop {
    raw: *mut uv_loop_t,
}

impl Loop {
    pub fn default() -> Result<Self, Errno> {
        let raw = unsafe { uv_default_loop() };
        if raw.is_null() {
            Err(Errno::ENOMEM)
        } else {
            Ok(Self { raw })
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
}

impl FromInner<*mut uv_loop_t> for Loop {
    fn from_inner(raw: *mut uv_loop_t) -> Self {
        Self { raw }
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

impl IntoInner<uv_run_mode> for RunMode {
    fn into_inner(self) -> uv_run_mode {
        match self {
            RunMode::DEFAULT => uv::uv_run_mode_UV_RUN_DEFAULT,
            RunMode::ONCE => uv::uv_run_mode_UV_RUN_ONCE,
            RunMode::NOWAIT => uv::uv_run_mode_UV_RUN_NOWAIT,
        }
    }
}
