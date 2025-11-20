use std::{
    alloc::{Layout, alloc},
    error::Error,
    fmt::Debug,
    ptr::copy_nonoverlapping,
};

use crate::{
    inners::IntoInner,
    uv::{Errno, uv_buf_init, uv_buf_t},
};

pub trait Buf: Debug + Clone + Copy + IntoInner<*mut uv_buf_t> {}

#[derive(Debug, Clone, Copy)]
pub struct MutBuf {
    raw: *mut uv_buf_t,
}

impl From<&str> for MutBuf {
    fn from(value: &str) -> Self {
        MutBuf::new_from_bytes(value.as_bytes()).unwrap()
    }
}

impl IntoInner<*mut uv_buf_t> for MutBuf {
    fn into_inner(self) -> *mut uv_buf_t {
        self.raw
    }
}

impl Buf for MutBuf {}

impl<T> IntoInner<Box<[uv_buf_t]>> for &[T]
where
    T: Buf,
{
    fn into_inner(self) -> Box<[uv_buf_t]> {
        let buf: Vec<uv_buf_t> = unsafe {
            self.iter()
                .map(|b| *(IntoInner::<*mut uv_buf_t>::into_inner(*b)))
                .collect()
        };
        buf.into_boxed_slice()
    }
}

impl MutBuf {
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

        Ok(MutBuf {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, len as u32) })), // uv_buf_t -> *mut uv_buf_t
        })
    }
}
