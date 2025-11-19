use std::alloc::{Layout, alloc};

use crate::uv::{Errno, uv_write_t};

pub struct WriteRequest {
    raw: *mut uv_write_t,
}

impl From<*mut uv_write_t> for WriteRequest {
    fn from(raw: *mut uv_write_t) -> Self {
        Self { raw }
    }
}

impl Into<*mut uv_write_t> for &WriteRequest {
    fn into(self) -> *mut uv_write_t {
        self.raw
    }
}

impl WriteRequest {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_write_t>();
        let raw = unsafe { alloc(layout) as *mut uv_write_t };
        if raw.is_null() {
            Err(Errno::ENOMEM)
        } else {
            Ok(Self { raw })
        }
    }
}

pub(crate) extern "C" fn uv_write_cb(_req: *mut uv_write_t, _status: std::os::raw::c_int) {}
