use std::alloc::{Layout, alloc, dealloc};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, Loop, uv_timer_init, uv_timer_start, uv_timer_t},
};

pub struct Timer {
    raw: *mut uv_timer_t,
}

impl FromInner<*mut uv_timer_t> for Timer {
    fn from_inner(raw: *mut uv_timer_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_timer_t> for &Timer {
    fn into_inner(self) -> *mut uv_timer_t {
        self.raw
    }
}

impl Timer {
    fn new(r#loop: &Loop) -> Result<Self, Errno> {
        let layout = Layout::new::<uv_timer_t>();
        let raw = unsafe { alloc(layout) as *mut uv_timer_t };
        if raw.is_null() {
            return Err(Errno::ENOMEM);
        }

        let result = unsafe { uv_timer_init(r#loop.into_inner(), raw) };
        if result < 0 {
            unsafe { dealloc(raw as *mut u8, layout) };
            return Err(Errno::from_inner(result));
        }
        Ok(Self { raw })
    }

    fn start(&mut self, timeout: u64, repeat: u64) -> Result<(), Errno> {
        let result = unsafe { uv_timer_start(self.raw, None, timeout, repeat) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }
}

impl Loop {
    fn new_timer(&self) -> Result<Timer, Errno> {
        Timer::new(self)
    }
}
