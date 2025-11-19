use std::{
    alloc::{Layout, alloc},
    error::Error,
    ptr::copy_nonoverlapping,
};

use crate::uv::{Errno, uv_buf_init, uv_buf_t};

#[derive(Clone)]
pub struct Buf {
    raw: *mut uv_buf_t,
}

impl From<&str> for Buf {
    fn from(value: &str) -> Self {
        Buf::new_from_bytes(value.as_bytes()).unwrap()
    }
}

impl Into<*mut uv_buf_t> for &Buf {
    fn into(self) -> *mut uv_buf_t {
        self.raw
    }
}

impl Buf {
    fn new_from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        let len = bytes.len();
        let baselen = len + 1; // null terminator
        let layout = Layout::array::<std::os::raw::c_char>(baselen)?;
        let base = unsafe { alloc(layout) as *mut std::os::raw::c_char };
        if base.is_null() {
            return Err(Box::new(Errno::ENOMEM));
        }

        unsafe {
            copy_nonoverlapping(bytes.as_ptr() as *mut i8, base, len);
            *base.offset(len as isize) = '\0' as i8;
        }

        Ok(Buf {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, len as u32) })), // uv_buf_t -> *mut uv_buf_t
        })
    }
}
