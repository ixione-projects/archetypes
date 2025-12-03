// mod

pub(crate) mod check;
pub(crate) use check::*;

pub(crate) mod stream;
pub(crate) use stream::*;

use std::{
    any::{Any, TypeId},
    ffi::CStr,
    os::raw::c_void,
    ptr::{copy_nonoverlapping, null_mut},
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        self, Buf, Loop, uv_buf_t, uv_check_t, uv_close, uv_handle_get_data, uv_handle_get_loop,
        uv_handle_get_type, uv_handle_set_data, uv_handle_t, uv_handle_type, uv_handle_type_name,
        uv_is_active, uv_is_closing, uv_stream_t,
    },
};

// type

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleType {
    UNKNOWN_HANDLE,
    ASYNC,
    CHECK,
    FS_EVENT,
    FS_POLL,
    HANDLE,
    IDLE,
    NAMED_PIPE,
    POLL,
    PREPARE,
    PROCESS,
    STREAM,
    TCP,
    TIMER,
    TTY,
    UDP,
    SIGNAL,
    FILE,
    HANDLE_TYPE_MAX,
}

pub struct AllocCallback<'a>(pub Box<dyn FnMut(&'a Handle, usize) -> Option<Buf> + 'a>);
pub struct CloseCallback<'a>(pub Box<dyn FnMut(&'a Handle) + 'a>);

#[repr(C)]
pub struct HandleContext<'a> {
    alloc_cb: Option<AllocCallback<'a>>,
    close_cb: Option<CloseCallback<'a>>,
    data: *mut c_void,
}

pub trait IHandleContext<'a> {
    fn into_handle_context(self) -> HandleContext<'a>;
}

#[derive(Debug, Clone, Copy)]
pub struct Handle {
    raw: *mut uv_handle_t,
}

pub trait IHandle: Copy {
    fn into_handle(self) -> Handle;

    fn drop_handle(self);

    fn active(&self) -> bool {
        self.into_handle().active()
    }

    fn closing(&self) -> bool {
        self.into_handle().closing()
    }

    fn close<'a, CCB>(&mut self, close_cb: CCB)
    where
        CCB: Into<CloseCallback<'a>>,
    {
        self.into_handle().close(close_cb);
    }

    fn get_loop(&self) -> Loop {
        self.into_handle().get_loop()
    }

    fn get_type(&self) -> HandleType {
        self.into_handle().get_type()
    }
}

// fn

pub(crate) fn init_handle(raw: *mut uv_handle_t) {
    unsafe { uv_handle_set_data(raw, null_mut()) };
}

pub(crate) unsafe extern "C" fn uv_alloc_cb(
    handle: *mut uv_handle_t,
    suggested_size: usize,
    buf: *mut uv_buf_t,
) {
    (*buf).base = null_mut();
    (*buf).len = 0;

    let handle = Handle::from_inner(handle);
    if let Some(context) = handle.get_context::<HandleContext>() {
        if let Some(ref mut alloc_cb) = context.alloc_cb {
            if let Some(new_buf) = alloc_cb.0(&handle, suggested_size) {
                (*buf).len = new_buf.len();
                let new_buf_ptr = new_buf.into_inner();
                copy_nonoverlapping(new_buf_ptr, buf, 1);
                drop(Box::from_raw(new_buf_ptr));
            }
        }
    }
}

pub(crate) unsafe extern "C" fn uv_close_cb(handle: *mut uv_handle_t) {
    let mut handle = Handle::from_inner(handle);
    if let Some(context) = handle.get_context::<HandleContext>() {
        if let Some(ref mut close_cb) = context.close_cb {
            close_cb.0(&handle);
        }
    }
    handle.drop_context();
    handle.drop_handle();
}

// impl

impl HandleType {
    pub fn name(&self) -> String {
        unsafe { CStr::from_ptr(uv_handle_type_name(self.into_inner())) }
            .to_string_lossy()
            .into_owned()
    }
}

impl<'a> IHandleContext<'a> for HandleContext<'a> {
    fn into_handle_context(self) -> HandleContext<'a> {
        self
    }
}

impl Handle {
    pub fn active(&self) -> bool {
        unsafe { uv_is_active(self.raw) != 0 }
    }

    pub fn closing(&self) -> bool {
        unsafe { uv_is_closing(self.raw) != 0 }
    }

    pub fn close<'a, CCB>(&mut self, close_cb: CCB)
    where
        CCB: Into<CloseCallback<'a>>,
    {
        match unsafe { self.get_context::<HandleContext>() } {
            Some(ref mut context) => {
                context.close_cb = Some(close_cb.into());
            }
            None => {
                self.set_context(HandleContext {
                    alloc_cb: None,
                    close_cb: Some(close_cb.into()),
                    data: null_mut(),
                });
            }
        };

        unsafe { uv_close(self.raw, Some(uv_close_cb)) };
    }

    pub fn get_loop(&self) -> Loop {
        Loop::from_inner(unsafe { uv_handle_get_loop(self.raw) })
    }

    pub fn get_type(&self) -> HandleType {
        HandleType::from_inner(unsafe { uv_handle_get_type(self.raw) })
    }

    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        if let Some(context) = unsafe { self.get_context::<HandleContext>() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "Handle::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
                        TypeId::of::<D>(),
                        (*context.data).type_id()
                    ))
            })
        } else {
            None
        }
    }

    pub fn set_data<D: 'static>(&mut self, data: D) {
        let data = Box::into_raw(Box::new(data)) as *mut c_void;
        match unsafe { self.get_context::<HandleContext>() } {
            Some(context) => context.data = data,
            None => {
                self.set_context(HandleContext {
                    alloc_cb: None,
                    close_cb: None,
                    data,
                });
            }
        }
    }

    pub(crate) unsafe fn get_context<'a, C: IHandleContext<'a>>(&self) -> Option<&mut C> {
        let context = uv_handle_get_data(self.raw);
        if context.is_null() {
            None
        } else {
            Some(&mut *(context as *mut C))
        }
    }

    pub(crate) fn set_context<'a, C: IHandleContext<'a>>(&mut self, context: C) {
        unsafe { uv_handle_set_data(self.raw, Box::into_raw(Box::new(context)) as *mut c_void) };
    }

    pub(crate) fn drop_context(&mut self) {
        if let Some(context) = unsafe { self.get_context::<HandleContext>() } {
            if !context.data.is_null() {
                drop(unsafe { Box::from_raw(context.data) })
            }
            drop(unsafe { Box::from_raw(context) })
        }
    }
}

impl IHandle for Handle {
    fn into_handle(self) -> Handle {
        Self { raw: self.raw }
    }

    fn drop_handle(self) {
        match self.get_type() {
            HandleType::CHECK => CheckHandle::from_inner(self.raw as *mut uv_check_t).drop_handle(),
            HandleType::STREAM => {
                StreamHandle::from_inner(self.raw as *mut uv_stream_t).drop_handle()
            }
            _ => panic!(
                "Handle::drop_handle: unexpected type [{}]",
                self.get_type().name()
            ),
        };
    }
}

// trait

impl<'a, Fn> From<Fn> for AllocCallback<'a>
where
    Fn: FnMut(&Handle, usize) -> Option<Buf> + 'a,
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

// from_inner/into_inner

impl FromInner<uv_handle_type> for HandleType {
    fn from_inner(value: uv_handle_type) -> Self {
        match value {
            uv::uv_handle_type_UV_UNKNOWN_HANDLE => HandleType::UNKNOWN_HANDLE,
            uv::uv_handle_type_UV_ASYNC => HandleType::ASYNC,
            uv::uv_handle_type_UV_CHECK => HandleType::CHECK,
            uv::uv_handle_type_UV_FS_EVENT => HandleType::FS_EVENT,
            uv::uv_handle_type_UV_FS_POLL => HandleType::FS_POLL,
            uv::uv_handle_type_UV_HANDLE => HandleType::HANDLE,
            uv::uv_handle_type_UV_IDLE => HandleType::IDLE,
            uv::uv_handle_type_UV_NAMED_PIPE => HandleType::NAMED_PIPE,
            uv::uv_handle_type_UV_POLL => HandleType::POLL,
            uv::uv_handle_type_UV_PREPARE => HandleType::PREPARE,
            uv::uv_handle_type_UV_PROCESS => HandleType::PROCESS,
            uv::uv_handle_type_UV_STREAM => HandleType::STREAM,
            uv::uv_handle_type_UV_TCP => HandleType::TCP,
            uv::uv_handle_type_UV_TIMER => HandleType::TIMER,
            uv::uv_handle_type_UV_TTY => HandleType::TTY,
            uv::uv_handle_type_UV_UDP => HandleType::UDP,
            uv::uv_handle_type_UV_SIGNAL => HandleType::SIGNAL,
            uv::uv_handle_type_UV_FILE => HandleType::FILE,
            uv::uv_handle_type_UV_HANDLE_TYPE_MAX => HandleType::HANDLE_TYPE_MAX,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<uv_handle_type> for HandleType {
    fn into_inner(self) -> uv_handle_type {
        match self {
            HandleType::UNKNOWN_HANDLE => uv::uv_handle_type_UV_UNKNOWN_HANDLE,
            HandleType::ASYNC => uv::uv_handle_type_UV_ASYNC,
            HandleType::CHECK => uv::uv_handle_type_UV_CHECK,
            HandleType::FS_EVENT => uv::uv_handle_type_UV_FS_EVENT,
            HandleType::FS_POLL => uv::uv_handle_type_UV_FS_POLL,
            HandleType::HANDLE => uv::uv_handle_type_UV_HANDLE,
            HandleType::IDLE => uv::uv_handle_type_UV_IDLE,
            HandleType::NAMED_PIPE => uv::uv_handle_type_UV_NAMED_PIPE,
            HandleType::POLL => uv::uv_handle_type_UV_POLL,
            HandleType::PREPARE => uv::uv_handle_type_UV_PREPARE,
            HandleType::PROCESS => uv::uv_handle_type_UV_PROCESS,
            HandleType::STREAM => uv::uv_handle_type_UV_STREAM,
            HandleType::TCP => uv::uv_handle_type_UV_TCP,
            HandleType::TIMER => uv::uv_handle_type_UV_TIMER,
            HandleType::TTY => uv::uv_handle_type_UV_TTY,
            HandleType::UDP => uv::uv_handle_type_UV_UDP,
            HandleType::SIGNAL => uv::uv_handle_type_UV_SIGNAL,
            HandleType::FILE => uv::uv_handle_type_UV_FILE,
            HandleType::HANDLE_TYPE_MAX => uv::uv_handle_type_UV_HANDLE_TYPE_MAX,
        }
    }
}

impl FromInner<*mut uv_handle_t> for Handle {
    fn from_inner(raw: *mut uv_handle_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_handle_t> for Handle {
    fn into_inner(self) -> *mut uv_handle_t {
        self.raw
    }
}
