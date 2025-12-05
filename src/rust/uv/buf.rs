use std::{
    alloc::{Layout, alloc, realloc},
    ffi::CStr,
    fmt::{Debug, Display},
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{Index, Range, RangeBounds},
    os::raw::c_char,
    ptr::{copy, copy_nonoverlapping, write, write_bytes},
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

pub struct BufIter<'a> {
    bytes: &'a [u8],
    off: usize,

    marker: PhantomData<&'a [u8]>,
}

// impl

impl Buf {
    pub fn new<T>(bytes: T) -> Self
    where
        T: Into<Vec<u8>>,
    {
        let vec = bytes.into();
        let len = vec.len();
        let baselen = len + 1; // null terminator

        match Layout::array::<c_char>(baselen) {
            Ok(layout) => {
                let base = unsafe { alloc(layout) as *mut c_char };
                if base.is_null() {
                    panic!("{}", Errno::ENOMEM);
                }

                unsafe {
                    copy_nonoverlapping(vec.as_ptr() as *mut i8, base, len);
                    write(base.add(len), 0);
                }

                Buf {
                    raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, baselen as u32) })),
                } // uv_buf_t -> *mut uv_buf_t
            }
            Err(_) => {
                panic!("{}", Errno::ENOMEM);
            }
        }
    }

    pub fn new_with_capacity(baselen: usize) -> Self {
        match Layout::array::<c_char>(baselen) {
            Ok(layout) => {
                let base = unsafe { alloc(layout) as *mut c_char };
                if base.is_null() {
                    panic!("{}", Errno::ENOMEM);
                }

                unsafe {
                    write_bytes(base, 0, baselen);
                }

                Buf {
                    raw: Box::into_raw(Box::new(unsafe { uv_buf_init(base, baselen as u32) })),
                } // uv_buf_t -> *mut uv_buf_t
            }
            Err(_) => {
                panic!("{}", Errno::ENOMEM);
            }
        }
    }

    pub fn join(bufs: &[Buf]) -> Self {
        let baselen = bufs.iter().map(|b| b.len()).sum();
        let result = Buf::new_with_capacity(baselen);
        let mut off = 0;
        for buf in bufs {
            let len = buf.len();
            unsafe { copy(buf.base(), result.base().add(off), len) }
            off += len;
        }
        result
    }

    pub fn resize(&mut self, newlen: usize) -> &mut Self {
        if newlen == self.len() {
            return self;
        }

        let layout = Layout::array::<c_char>(newlen).unwrap();
        let newbase = unsafe { realloc(self.base() as *mut u8, layout, newlen) };
        if newbase.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        unsafe {
            copy(self.base() as *mut u8, newbase, self.len());

            (*self.raw).base = newbase as *mut i8;
            (*self.raw).len = newlen;
        }

        self
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.raw).len }
    }

    pub(crate) fn base(&self) -> *mut i8 {
        unsafe { (*self.raw).base }
    }

    pub fn iter(&self) -> BufIter<'_> {
        BufIter {
            bytes: self.as_bytes(),
            off: 0,
            marker: PhantomData,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { from_raw_parts(self.base() as *const u8, self.len()) }
    }
}

// trait

impl Display for Buf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            unsafe { CStr::from_ptr(self.base()) }.to_string_lossy()
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
            copy_nonoverlapping(self.base() as *mut i8, base, len);
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

impl<'a> Iterator for BufIter<'a> {
    type Item = &'a u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.off < self.bytes.len() && self.bytes[self.off] != 0 {
            self.off += 1;
            Some(&self.bytes[self.off - 1])
        } else {
            None
        }
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
