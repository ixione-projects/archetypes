use crate::uv::{self, Errno, uv_default_loop, uv_errno_t, uv_loop_t, uv_run, uv_run_mode};

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum RunMode {
    DEFAULT,
    ONCE,
    NOWAIT,
}

impl From<uv_run_mode> for RunMode {
    fn from(value: uv_run_mode) -> Self {
        match value {
            uv::uv_run_mode_UV_RUN_DEFAULT => RunMode::DEFAULT,
            uv::uv_run_mode_UV_RUN_ONCE => RunMode::ONCE,
            uv::uv_run_mode_UV_RUN_NOWAIT => RunMode::NOWAIT,
            _ => unreachable!(),
        }
    }
}

impl Into<uv_run_mode> for RunMode {
    fn into(self) -> uv_run_mode {
        match self {
            RunMode::DEFAULT => uv::uv_run_mode_UV_RUN_DEFAULT,
            RunMode::ONCE => uv::uv_run_mode_UV_RUN_ONCE,
            RunMode::NOWAIT => uv::uv_run_mode_UV_RUN_NOWAIT,
        }
    }
}

pub struct Loop {
    raw: *mut uv_loop_t,
}

impl From<*mut uv_loop_t> for Loop {
    fn from(raw: *mut uv_loop_t) -> Self {
        Self { raw }
    }
}

impl Into<*mut uv_loop_t> for &Loop {
    fn into(self) -> *mut uv_loop_t {
        self.raw
    }
}

impl Into<*mut uv_loop_t> for &mut Loop {
    fn into(self) -> *mut uv_loop_t {
        self.raw
    }
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
        let result = unsafe { uv_run(self.into(), mode.into()) };
        if result < 0 {
            Err(Errno::from(result as uv_errno_t))
        } else {
            Ok(())
        }
    }
}
