use std::{marker::PhantomData, os::raw::c_void, ptr::null_mut};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Buf, uv_buf_t, uv_handle_get_data, uv_handle_set_data, uv_handle_t},
};

pub(crate) mod stream;
pub(crate) mod timer;

// TODO: if a type is not specified then infer MutBuf
pub struct AllocCallback<'a, B: Buf>(pub Box<dyn FnMut(&'a Handle<B>, usize) -> Option<B> + 'a>);

pub struct HandleContext<'a, B: Buf> {
    alloc_cb: Option<AllocCallback<'a, B>>,
}

pub trait IHandleContext<'a, B: Buf> {
    fn into_handle_context(self) -> HandleContext<'a, B>;
}

impl<'a, B: Buf> IHandleContext<'a, B> for HandleContext<'a, B> {
    fn into_handle_context(self) -> HandleContext<'a, B> {
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Handle<B: Buf> {
    raw: *mut uv_handle_t,
    _marker: PhantomData<B>,
}

pub trait IHandle<B: Buf> {
    fn into_handle(self) -> Handle<B>;
}

impl<B: Buf> IHandle<B> for Handle<B> {
    fn into_handle(self) -> Handle<B> {
        Self {
            raw: self.raw,
            _marker: self._marker,
        }
    }
}

pub(crate) fn init_handle(raw: *mut uv_handle_t) {
    unsafe { uv_handle_set_data(raw, null_mut()) };
}

impl<B: Buf> Handle<B> {
    pub(crate) fn from_raw(raw: *mut uv_handle_t) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    pub fn get_context<'a, C: IHandleContext<'a, B>>(&self) -> Option<&mut C> {
        let context = unsafe { uv_handle_get_data(self.raw) };
        if context.is_null() {
            None
        } else {
            Some(unsafe { &mut *(context as *mut C) })
        }
    }

    pub fn set_context<'a, C: IHandleContext<'a, B>>(&mut self, context: C) {
        unsafe { uv_handle_set_data(self.raw, Box::into_raw(Box::new(context)) as *mut c_void) };
    }
}

pub(crate) unsafe extern "C" fn uv_alloc_cb<B: Buf>(
    handle: *mut uv_handle_t,
    suggested_size: usize,
    buf: *mut uv_buf_t,
) {
    let handle = Handle::<B>::from_inner(handle);
    if let Some(context) = handle.get_context::<HandleContext<B>>() {
        if let Some(ref mut alloc_cb) = context.alloc_cb {
            if let Some(new_buf) = alloc_cb.0(&handle, suggested_size) {
                buf.copy_from_nonoverlapping(new_buf.into_inner(), 1);
                drop(Box::from_raw(new_buf.into_inner() as *mut uv_buf_t));
                return;
            }
        }
    }

    (*buf).base = null_mut();
    (*buf).len = 0;
}

impl<'a, Fn, B: Buf> From<Fn> for AllocCallback<'a, B>
where
    Fn: FnMut(&Handle<B>, usize) -> Option<B> + 'static,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a, B: Buf> From<()> for AllocCallback<'a, B> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _| None))
    }
}

impl<B: Buf> FromInner<*mut uv_handle_t> for Handle<B> {
    fn from_inner(raw: *mut uv_handle_t) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }
}

impl<B: Buf> IntoInner<*mut uv_handle_t> for &Handle<B> {
    fn into_inner(self) -> *mut uv_handle_t {
        self.raw
    }
}
