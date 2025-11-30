use std::{
    alloc::{Layout, alloc},
    error::Error,
    ffi::CStr,
    fmt::{Debug, Display},
    ops::{Index, Range, RangeBounds},
    os::raw::c_char,
    ptr::copy_nonoverlapping,
    slice::from_raw_parts,
};

use crate::{
    inners::{FromInner, IntoInner, NullPtrError},
    uv::{Errno, uv_buf_init, uv_buf_t},
};

pub trait Buf: Copy + FromInner<*const uv_buf_t> + IntoInner<*const uv_buf_t> {
    fn from_raw(raw: *const uv_buf_t) -> Self;

    fn len(&self) -> Result<usize, Box<dyn Error>> {
        let raw = self.into_inner();
        if raw.is_null() {
            Err(Box::new(NullPtrError()))
        } else {
            unsafe { Ok((*raw).len) }
        }
    }

    fn to_bytes(&self, len: usize) -> Result<&[u8], Box<dyn Error>> {
        let raw = self.into_inner();
        if raw.is_null() {
            Err(Box::new(NullPtrError()))
        } else {
            unsafe { Ok(from_raw_parts((*raw).base as *const u8, len)) }
        }
    }
}

fn base_alloc(baselen: usize) -> Result<*mut c_char, Box<dyn Error>> {
    let layout = Layout::array::<c_char>(baselen)?;
    let base = unsafe { alloc(layout) as *mut c_char };
    if base.is_null() {
        return Err(Box::new(Errno::ENOMEM));
    }
    Ok(base)
}

pub fn new_from_bytes<T: Buf>(bytes: &[u8]) -> Result<T, Box<dyn Error>> {
    let len = bytes.len();
    let baselen = len + 1; // null terminator

    let base = base_alloc(baselen)?;
    unsafe {
        copy_nonoverlapping(bytes.as_ptr() as *mut i8, base, len);
        *base.add(len) = '\0' as i8;
    }

    Ok(T::from_raw(Box::into_raw(Box::new(unsafe {
        uv_buf_init(base, baselen as u32)
    })))) // uv_buf_t -> *mut uv_buf_t
}

pub fn new_with_capacity<T: Buf>(baselen: usize) -> Result<T, Box<dyn Error>> {
    let base = base_alloc(baselen)?;

    Ok(T::from_raw(Box::into_raw(Box::new(unsafe {
        uv_buf_init(base, baselen as u32)
    })))) // uv_buf_t -> *mut uv_buf_t
}

// TODO: Clone should create a new allocation
#[derive(Debug, Clone, Copy)]
pub struct ConstBuf {
    raw: *const uv_buf_t,
}

// TODO: Clone should create a new allocation
#[derive(Debug, Clone, Copy)]
pub struct MutBuf {
    raw: *mut uv_buf_t,
}

impl MutBuf {
    pub fn as_const(self) -> ConstBuf {
        ConstBuf { raw: self.raw }
    }
}

impl Buf for ConstBuf {
    fn from_raw(raw: *const uv_buf_t) -> Self {
        Self { raw }
    }
}

impl Buf for MutBuf {
    fn from_raw(raw: *const uv_buf_t) -> Self {
        Self {
            raw: raw as *mut uv_buf_t,
        }
    }
}

impl Display for ConstBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Into::<String>::into(*self))
    }
}

impl Display for MutBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Into::<String>::into(*self))
    }
}

impl Index<usize> for ConstBuf {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*((*self.raw).base.add(index) as *const u8) }
    }
}

impl Index<usize> for MutBuf {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*((*self.raw).base.add(index) as *mut u8) }
    }
}

impl Index<Range<usize>> for ConstBuf {
    type Output = [u8];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        let start = match index.start_bound() {
            std::ops::Bound::Included(i) => *i,
            std::ops::Bound::Excluded(i) => *i + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match index.end_bound() {
            std::ops::Bound::Included(i) => *i + 1,
            std::ops::Bound::Excluded(i) => *i,
            std::ops::Bound::Unbounded => self.len().unwrap(),
        };

        unsafe { from_raw_parts((*self.raw).base.add(start) as *const u8, end - start) }
    }
}

impl Index<Range<usize>> for MutBuf {
    type Output = [u8];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        let start = match index.start_bound() {
            std::ops::Bound::Included(i) => *i,
            std::ops::Bound::Excluded(i) => *i + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match index.end_bound() {
            std::ops::Bound::Included(i) => *i + 1,
            std::ops::Bound::Excluded(i) => *i,
            std::ops::Bound::Unbounded => self.len().unwrap(),
        };

        unsafe { from_raw_parts((*self.raw).base.add(start) as *const u8, end - start) }
    }
}

impl From<String> for ConstBuf {
    fn from(value: String) -> Self {
        new_from_bytes(value.as_bytes()).unwrap()
    }
}

impl From<String> for MutBuf {
    fn from(value: String) -> Self {
        new_from_bytes(value.as_bytes()).unwrap()
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

impl From<Box<[u8]>> for ConstBuf {
    fn from(value: Box<[u8]>) -> Self {
        new_from_bytes(&value).unwrap()
    }
}

impl From<Box<[u8]>> for MutBuf {
    fn from(value: Box<[u8]>) -> Self {
        new_from_bytes(&value).unwrap()
    }
}

impl From<Vec<u8>> for ConstBuf {
    fn from(value: Vec<u8>) -> Self {
        new_from_bytes(&value).unwrap()
    }
}

impl From<Vec<u8>> for MutBuf {
    fn from(value: Vec<u8>) -> Self {
        new_from_bytes(&value).unwrap()
    }
}

impl Into<String> for ConstBuf {
    fn into(self) -> String {
        unsafe {
            String::from_utf8(
                from_raw_parts((*self.raw).base as *const u8, (*self.raw).len).to_vec(),
            )
            .unwrap()
        }
    }
}

impl Into<String> for MutBuf {
    fn into(self) -> String {
        unsafe {
            String::from_utf8(from_raw_parts((*self.raw).base as *mut u8, (*self.raw).len).to_vec())
                .unwrap()
        }
    }
}

impl<T> IntoInner<Box<[uv_buf_t]>> for &[T]
where
    T: Buf,
{
    fn into_inner(self) -> Box<[uv_buf_t]> {
        let mut buf = Vec::new();
        unsafe {
            for b in self {
                buf.push(*(T::into_inner(*b)));
            }
        };
        buf.into_boxed_slice()
    }
}

impl FromInner<*const uv_buf_t> for ConstBuf {
    fn from_inner(raw: *const uv_buf_t) -> Self {
        Self { raw }
    }
}

impl FromInner<*mut uv_buf_t> for ConstBuf {
    fn from_inner(raw: *mut uv_buf_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*const uv_buf_t> for ConstBuf {
    fn into_inner(self) -> *const uv_buf_t {
        self.raw
    }
}

impl FromInner<*const uv_buf_t> for MutBuf {
    fn from_inner(raw: *const uv_buf_t) -> Self {
        Self {
            raw: raw as *mut uv_buf_t,
        }
    }
}

impl FromInner<*mut uv_buf_t> for MutBuf {
    fn from_inner(raw: *mut uv_buf_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*const uv_buf_t> for MutBuf {
    fn into_inner(self) -> *const uv_buf_t {
        self.raw
    }
}
