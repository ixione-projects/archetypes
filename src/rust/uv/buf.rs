use std::{
    alloc::{Layout, alloc, dealloc, realloc},
    ffi::CStr,
    fmt::{Debug, Display},
    mem::ManuallyDrop,
    ops::{Deref, DerefMut, Index, Range, RangeBounds},
    os::raw::c_char,
    ptr::{copy, copy_nonoverlapping, null_mut, write, write_bytes},
    slice::{from_raw_parts, from_raw_parts_mut},
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, uv_buf_init, uv_buf_t},
};

// type

pub struct Buf {
    raw: *mut uv_buf_t,
}

pub trait ConvertBuf {
    fn to_buf(&self) -> Buf;
}

// fn

pub(crate) unsafe fn alloc_base(baselen: usize) -> *mut c_char {
    match Layout::array::<c_char>(baselen) {
        Ok(layout) => {
            let base = unsafe { alloc(layout) as *mut c_char };
            if base.is_null() {
                panic!("{}", Errno::ENOMEM);
            }

            unsafe {
                write_bytes(base, 0, baselen);
            }

            base
        }
        Err(_) => {
            panic!("{}", Errno::ENOMEM);
        }
    }
}

pub(crate) unsafe fn realloc_base(base: *mut c_char, oldlen: usize, newlen: usize) -> *mut c_char {
    match Layout::array::<c_char>(newlen) {
        Ok(layout) => {
            let newbase = unsafe { realloc(base as *mut u8, layout, newlen) };
            if newbase.is_null() {
                panic!("{}", Errno::ENOMEM);
            }

            if newlen > oldlen {
                unsafe {
                    write_bytes(newbase.add(oldlen), 0, newlen - oldlen);
                }
            }

            newbase as *mut c_char
        }
        Err(_) => {
            panic!("{}", Errno::ENOMEM);
        }
    }
}

pub(crate) unsafe fn dealloc_base(base: *mut c_char, baselen: usize) {
    match Layout::array::<c_char>(baselen) {
        Ok(layout) => {
            unsafe { dealloc(base as *mut u8, layout) };
        }
        Err(_) => {
            panic!("{}", Errno::ENOMEM);
        }
    }
}

// impl

impl Buf {
    pub fn new() -> Self {
        let layout = Layout::new::<uv_buf_t>();
        let raw = unsafe { alloc(layout) as *mut uv_buf_t };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        unsafe {
            (*raw).base = null_mut();
            (*raw).len = 0; // uninitialized
        }

        Self { raw }
    }

    pub fn new_with_len(baselen: usize) -> Self {
        let base = unsafe { alloc_base(baselen) };
        Buf {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, baselen as u32) })),
        } // uv_buf_t -> *mut uv_buf_t
    }

    pub fn join(bufs: &[Buf]) -> Self {
        let baselen = bufs.iter().map(|b| b.len()).sum();
        let mut result = Buf::new_with_len(baselen);
        let mut off = 0;
        bufs.iter()
            .filter(|buf| buf.is_initialized())
            .for_each(|buf| {
                let len = buf.non_null_len();
                unsafe {
                    copy(buf.base(), result.base().add(off), len);

                    dealloc_base(buf.base(), buf.len());
                    (*buf.raw).base = null_mut();
                    (*buf.raw).len = 0;
                }
                off += len;
            });
        result.resize(off + 1); // null terminator
        unsafe {
            write(result.base().add(off), 0);
        }
        result
    }

    pub fn append(&mut self, other: &Self) -> &mut Self {
        let selflen = self.non_null_len();
        let otherlen = other.non_null_len();
        self.resize(selflen + otherlen + 1);

        unsafe {
            copy(other.base(), self.base().add(selflen), otherlen);
            write(self.base().add(selflen + otherlen), 0);

            dealloc_base(other.base(), other.len());
            (*other.raw).base = null_mut();
            (*other.raw).len = 0;
        }

        self
    }

    pub fn resize(&mut self, newlen: usize) -> &mut Self {
        if newlen == self.len() {
            return self;
        }

        unsafe {
            if !self.is_initialized() {
                (*self.raw).base = alloc_base(newlen);
                (*self.raw).len = newlen;
            } else {
                let newbase = realloc_base(self.base(), self.len(), newlen);

                (*self.raw).base = newbase as *mut i8;
                (*self.raw).len = newlen;
            }
        }

        self
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.raw).len }
    }

    pub(crate) fn base(&self) -> *mut i8 {
        unsafe { (*self.raw).base }
    }

    pub fn as_bytes(&self) -> &[u8] {
        if !self.is_initialized() {
            &[]
        } else {
            unsafe { from_raw_parts(self.base() as *const u8, self.len()) }
        }
    }

    pub fn as_bytes_mut(&self) -> &mut [u8] {
        if !self.is_initialized() {
            &mut []
        } else {
            unsafe { from_raw_parts_mut(self.base() as *mut u8, self.len()) }
        }
    }

    #[inline]
    pub(crate) fn non_null_len(&self) -> usize {
        if !self.is_initialized() {
            0
        } else {
            // NOTE: if None then Buf was not properly initialized
            self.iter().position(|ch| ch == &b'\0').unwrap()
        }
    }

    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.len() != 0
    }
}

// trait

impl ConvertBuf for [u8] {
    fn to_buf(&self) -> Buf {
        Buf::from(self)
    }
}

impl Display for Buf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.is_initialized() {
            write!(f, "")
        } else {
            write!(f, "{:?}", unsafe { CStr::from_ptr(self.base()) })
        }
    }
}

impl Debug for Buf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buf").field("raw", &self.raw).finish()
    }
}

impl Clone for Buf {
    fn clone(&self) -> Self {
        let len = self.len();
        let layout = Layout::array::<c_char>(len).unwrap();
        let base = unsafe { alloc(layout) as *mut c_char };
        if base.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        unsafe {
            copy_nonoverlapping(self.base() as *mut i8, base, len);
        }

        Self {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, len as u32) })),
        }
    }
}

impl Copy for Buf {} // copy-safe because Buf is ownerless

impl Index<usize> for Buf {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*((*self.raw).base.add(index) as *const u8) }
    }
}

impl Index<Range<usize>> for Buf {
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
            std::ops::Bound::Unbounded => self.len(),
        };

        unsafe { from_raw_parts((*self.raw).base.add(start) as *const u8, end - start) }
    }
}

impl Deref for Buf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl DerefMut for Buf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_bytes_mut()
    }
}

impl<T> From<T> for Buf
where
    T: Into<Vec<u8>>,
{
    fn from(bytes: T) -> Self {
        let vec = bytes.into();
        let len = vec.len();
        let baselen = len + 1; // null terminator

        let base = unsafe { alloc_base(baselen) };
        unsafe {
            copy_nonoverlapping(vec.as_ptr() as *mut i8, base, len);
            write(base.add(len), 0);
        }

        Buf {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, baselen as u32) })),
        } // uv_buf_t -> *mut uv_buf_t
    }
}

// inner

impl FromInner<*mut uv_buf_t> for Buf {
    fn from_inner(raw: *mut uv_buf_t) -> Self {
        if unsafe { *(*raw).base.add((*raw).len) } != 0 {
            panic!("expected uv_buf_t to be initialized")
        }

        Self { raw }
    }
}

impl IntoInner<*mut uv_buf_t> for Buf {
    fn into_inner(self) -> *mut uv_buf_t {
        self.raw
    }
}

impl FromInner<(*mut uv_buf_t, usize)> for &[Buf] {
    fn from_inner((base, len): (*mut uv_buf_t, usize)) -> Self {
        let mut buf = Vec::with_capacity(len);
        for mut v in unsafe { Vec::from_raw_parts(base, len, len) } {
            buf.push(Buf::from_inner(&mut v));
        }
        unsafe { &*Box::into_raw(buf.into_boxed_slice()) }
    }
}

impl IntoInner<(*mut uv_buf_t, usize)> for &[Buf] {
    fn into_inner(self) -> (*mut uv_buf_t, usize) {
        let mut buf = Vec::with_capacity(self.len());
        unsafe {
            for b in self {
                buf.push(*b.into_inner());
            }
        };
        let mut buf = ManuallyDrop::new(buf);
        (buf.as_mut_ptr(), buf.len())
    }
}
