use std::{
    alloc::{Layout, alloc},
    cmp::{self, max, min},
    error::Error,
    ffi::CStr,
    fmt::{Debug, Display},
    mem::ManuallyDrop,
    ops::{Index, Range, RangeBounds},
    os::raw::c_char,
    ptr::{copy_nonoverlapping, write, write_bytes},
    slice::from_raw_parts,
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, uv_buf_init, uv_buf_t},
};

// type

pub struct Buf {
    raw: *mut uv_buf_t,
}

// impl

impl Buf {
    pub fn new<T>(bytes: T) -> Result<Self, Box<dyn Error>>
    where
        T: Into<Vec<u8>>,
    {
        let vec = bytes.into();
        let len = vec.len();
        let baselen = len + 1; // null terminator

        let layout = Layout::array::<c_char>(baselen)?;
        let base = unsafe { alloc(layout) as *mut c_char };
        if base.is_null() {
            return Err(Box::new(Errno::ENOMEM));
        }

        unsafe {
            copy_nonoverlapping(vec.as_ptr() as *mut i8, base, len);
            write(base.add(len), 0);
        }

        Ok(Buf {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, baselen as u32) })),
        }) // uv_buf_t -> *mut uv_buf_t
    }

    pub fn new_with_capacity(baselen: usize) -> Result<Self, Box<dyn Error>> {
        let layout = Layout::array::<c_char>(baselen)?;
        let base = unsafe { alloc(layout) as *mut c_char };
        if base.is_null() {
            return Err(Box::new(Errno::ENOMEM));
        }

        unsafe {
            write_bytes(base, 0, baselen);
        }

        Ok(Buf {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, baselen as u32) })),
        }) // uv_buf_t -> *mut uv_buf_t
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.raw).len }
    }

    pub fn as_bytes(&self, len: usize) -> &[u8] {
        &(unsafe { from_raw_parts(self.as_ptr() as *const u8, self.len()) })[..len]
    }

    pub(crate) fn as_ptr(&self) -> *mut i8 {
        unsafe { (*self.raw).base }
    }
}

// trait

impl Display for Buf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            unsafe { CStr::from_ptr(self.as_ptr()) }.to_string_lossy()
        )
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
            copy_nonoverlapping(self.as_ptr() as *mut i8, base, len);
        }

        Self {
            raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, len as u32) })),
        }
    }
}

impl Copy for Buf {}

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

// from_inner/into_inner

impl FromInner<*mut uv_buf_t> for Buf {
    fn from_inner(raw: *mut uv_buf_t) -> Self {
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
        Vec::leak(buf)
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
