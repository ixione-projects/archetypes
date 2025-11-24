use std::{
    alloc::{Layout, alloc},
    error::Error,
    fmt::Debug,
    os::raw::c_char,
    ptr::copy_nonoverlapping,
    slice::from_raw_parts,
};

use crate::{
    inners::{IntoInner, NullPtrError},
    uv::{Errno, uv_buf_init, uv_buf_t},
};

pub trait Buf: Copy + IntoInner<*const uv_buf_t> {
    fn from_raw(raw: *const uv_buf_t) -> Self;

    fn to_bytes(&self, len: usize) -> Result<&[u8], Box<dyn Error>> {
        let raw = self.into_inner();
        if raw.is_null() {
            Err(Box::new(NullPtrError()))
        } else {
            unsafe { Ok(from_raw_parts((*raw).base as *const u8, len)) }
        }
    }
}

pub fn new_from_bytes<T: Buf>(bytes: &[u8]) -> Result<T, Box<dyn Error>> {
    let len = bytes.len();
    let baselen = len + 1; // null terminator
    let layout = Layout::array::<c_char>(baselen)?;
    let base = unsafe { alloc(layout) as *mut c_char };
    if base.is_null() {
        return Err(Box::new(Errno::ENOMEM));
    }

    unsafe {
        copy_nonoverlapping(bytes.as_ptr() as *mut i8, base, len);
        *base.offset(len as isize) = '\0' as i8;
    }

    Ok(T::from_raw(Box::into_raw(Box::new(unsafe {
        uv_buf_init(base, baselen as u32)
    })))) // uv_buf_t -> *mut uv_buf_t
}

pub fn new_with_capacity<T: Buf>(baselen: usize) -> Result<T, Box<dyn Error>> {
    let layout = Layout::array::<c_char>(baselen)?;
    let base = unsafe { alloc(layout) as *mut c_char };
    if base.is_null() {
        return Err(Box::new(Errno::ENOMEM));
    }

    Ok(T::from_raw(Box::into_raw(Box::new(unsafe {
        uv_buf_init(base, baselen as u32)
    })))) // uv_buf_t -> *mut uv_buf_t
}

// TODO: Clone should create a new allocation
#[derive(Debug, Clone, Copy)]
pub struct ConstBuf {
    raw: *const uv_buf_t,
}

impl Buf for ConstBuf {
    fn from_raw(raw: *const uv_buf_t) -> Self {
        Self { raw }
    }
}

// TODO: Clone should create a new allocation
#[derive(Debug, Clone, Copy)]
pub struct MutBuf {
    raw: *mut uv_buf_t,
}

impl Buf for MutBuf {
    fn from_raw(raw: *const uv_buf_t) -> Self {
        Self {
            raw: raw as *mut uv_buf_t,
        }
    }
}

impl From<&str> for ConstBuf {
    fn from(value: &str) -> Self {
        new_from_bytes(value.as_bytes()).unwrap()
    }
}

impl From<&str> for MutBuf {
    fn from(value: &str) -> Self {
        new_from_bytes(value.as_bytes()).unwrap()
    }
}

impl<T> IntoInner<Box<[uv_buf_t]>> for &[T]
where
    T: Buf,
{
    fn into_inner(self) -> Box<[uv_buf_t]> {
        let buf: Vec<uv_buf_t> = unsafe {
            self.iter()
                .map(|b| *(IntoInner::<*const uv_buf_t>::into_inner(*b)))
                .collect()
        };
        buf.into_boxed_slice()
    }
}

impl IntoInner<*const uv_buf_t> for ConstBuf {
    fn into_inner(self) -> *const uv_buf_t {
        self.raw
    }
}

impl IntoInner<*const uv_buf_t> for MutBuf {
    fn into_inner(self) -> *const uv_buf_t {
        self.raw
    }
}
