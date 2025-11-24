use std::{os::raw::c_void, ptr::null_mut};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{MutBuf, uv_buf_t, uv_handle_get_data, uv_handle_set_data, uv_handle_t},
};

pub(crate) mod stream;

pub struct AllocCallback<'a>(pub Box<dyn FnMut(&'a Handle, usize) -> Option<MutBuf> + 'a>);

pub struct HandleContext<'a> {
    alloc_cb: Option<AllocCallback<'a>>,
}

pub trait IHandleContext<'a> {
    fn into_handle_context(self) -> HandleContext<'a>;
}

impl<'a> IHandleContext<'a> for HandleContext<'a> {
    fn into_handle_context(self) -> HandleContext<'a> {
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Handle {
    raw: *mut uv_handle_t,
}

pub trait IHandle {
    fn into_handle(self) -> Handle;
}

impl IHandle for Handle {
    fn into_handle(self) -> Handle {
        Self { raw: self.raw }
    }
}

pub(crate) fn init_handle(raw: *mut uv_handle_t) {
    unsafe { uv_handle_set_data(raw, null_mut()) };
}

impl Handle {
    pub(crate) fn from_raw(raw: *mut uv_handle_t) -> Self {
        Self { raw }
    }

    pub fn get_context<'a, C: IHandleContext<'a>>(&self) -> Option<&mut C> {
        let context = unsafe { uv_handle_get_data(self.raw) };
        if context.is_null() {
            None
        } else {
            Some(unsafe { &mut *(context as *mut C) })
        }
    }

    pub fn set_context<'a, C: IHandleContext<'a>>(&mut self, context: C) {
        unsafe { uv_handle_set_data(self.raw, Box::into_raw(Box::new(context)) as *mut c_void) };
    }
}

pub(crate) unsafe extern "C" fn uv_alloc_cb(
    handle: *mut uv_handle_t,
    suggested_size: usize,
    buf: *mut uv_buf_t,
) {
    let handle = Handle::from_inner(handle);
    if let Some(context) = handle.get_context::<HandleContext>() {
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

impl<'a, Fn> From<Fn> for AllocCallback<'a>
where
    Fn: FnMut(&Handle, usize) -> Option<MutBuf> + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for AllocCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _| None))
    }
}

impl FromInner<*mut uv_handle_t> for Handle {
    fn from_inner(raw: *mut uv_handle_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_handle_t> for &Handle {
    fn into_inner(self) -> *mut uv_handle_t {
        self.raw
    }
}
