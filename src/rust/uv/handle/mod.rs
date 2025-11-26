use std::{os::raw::c_void, ptr::null_mut};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        HandleType, Loop, MutBuf, check::CheckHandle, stream::StreamHandle, uv_buf_t, uv_check_t,
        uv_close, uv_handle_get_data, uv_handle_get_loop, uv_handle_get_type, uv_handle_set_data,
        uv_handle_t, uv_is_active, uv_is_closing, uv_stream_t,
    },
};

pub(crate) mod check;
pub(crate) mod stream;

pub struct AllocCallback<'a>(pub Box<dyn FnMut(&'a Handle, usize) -> Option<MutBuf> + 'a>);
pub struct CloseCallback<'a>(pub Box<dyn FnMut(&'a Handle) + 'a>);

pub struct HandleContext<'a> {
    alloc_cb: Option<AllocCallback<'a>>,
    close_cb: Option<CloseCallback<'a>>,
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

pub trait IHandle: Copy {
    fn into_handle(self) -> Handle;
    fn free_handle(self);

    fn active(&self) -> bool {
        unsafe { uv_is_active(self.into_handle().raw) != 0 }
    }

    fn closing(&self) -> bool {
        unsafe { uv_is_closing(self.into_handle().raw) != 0 }
    }

    fn close<'a, CCB>(&mut self, close_cb: CCB)
    where
        CCB: Into<CloseCallback<'a>>,
    {
        match self.get_context::<HandleContext>() {
            Some(ref mut context) => {
                context.close_cb = Some(close_cb.into());
            }
            None => {
                let new_context = HandleContext {
                    alloc_cb: None,
                    close_cb: Some(close_cb.into()),
                };
                self.set_context(new_context);
            }
        };

        unsafe { uv_close(self.into_handle().raw, Some(uv_close_cb)) };
    }

    fn get_loop(&self) -> Loop {
        Loop::from_inner(unsafe { uv_handle_get_loop(self.into_handle().raw) })
    }

    fn get_type(&self) -> HandleType {
        HandleType::from_inner(unsafe { uv_handle_get_type(self.into_handle().raw) })
    }

    fn get_context<'a, C: IHandleContext<'a>>(&self) -> Option<&mut C> {
        let context = unsafe { uv_handle_get_data(self.into_handle().raw) };
        if context.is_null() {
            None
        } else {
            Some(unsafe { &mut *(context as *mut C) })
        }
    }

    fn set_context<'a, C: IHandleContext<'a>>(&mut self, context: C) {
        unsafe {
            uv_handle_set_data(
                self.into_handle().raw,
                Box::into_raw(Box::new(context)) as *mut c_void,
            )
        };
    }

    fn free_context(&mut self) {
        let context = unsafe { uv_handle_get_data(self.into_handle().raw) };
        if !context.is_null() {
            unsafe { drop(Box::from_raw(context)) }
        }
    }
}

impl IHandle for Handle {
    fn into_handle(self) -> Handle {
        Self { raw: self.raw }
    }

    fn free_handle(self) {
        match self.get_type() {
            HandleType::CHECK => CheckHandle::from_inner(self.raw as *mut uv_check_t).free_handle(),
            HandleType::STREAM => {
                StreamHandle::from_inner(self.raw as *mut uv_stream_t).free_handle()
            }
            _ => panic!("unexpected handle type [{}]", self.get_type().name()),
        };
    }
}

pub(crate) fn init_handle(raw: *mut uv_handle_t) {
    unsafe { uv_handle_set_data(raw, null_mut()) };
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

pub(crate) unsafe extern "C" fn uv_close_cb(handle: *mut uv_handle_t) {
    let mut handle = Handle::from_inner(handle);
    if let Some(context) = handle.get_context::<HandleContext>() {
        if let Some(ref mut close_cb) = context.close_cb {
            close_cb.0(&handle);
        }
    }
    handle.free_context();
    handle.free_handle();
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

impl<'a, Fn> From<Fn> for CloseCallback<'a>
where
    Fn: FnMut(&Handle) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for CloseCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_| ()))
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
