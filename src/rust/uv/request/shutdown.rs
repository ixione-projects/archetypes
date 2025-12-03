use std::{
    alloc::{Layout, alloc, dealloc},
    any::{Any, TypeId},
    os::raw::{c_int, c_void},
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, IRequest, uv_req_t, uv_shutdown_t},
};

// super

impl<'a> super::IRequestContext for ShutdownContext<'a> {
    fn into_request_context(self) -> super::RequestContext {
        super::RequestContext::from(self)
    }
}

impl<'a> super::IRequest for ShutdownRequest {
    fn into_request(self) -> super::Request {
        super::Request::from_inner(self.raw as *mut uv_req_t)
    }

    fn drop_request(self) {
        let layout = Layout::new::<uv_shutdown_t>();
        unsafe { dealloc(self.raw as *mut u8, layout) };
    }
}

// type

pub struct ShutdownCallback<'a>(pub Box<dyn FnMut(ShutdownRequest, Result<(), Errno>) + 'a>);

#[repr(C)]
pub struct ShutdownContext<'a> {
    pub(crate) data: *mut c_void,
    pub(crate) shutdown_cb: Option<ShutdownCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ShutdownRequest {
    raw: *mut uv_shutdown_t,
}

// fn

pub(crate) unsafe extern "C" fn uv_shutdown_cb(req: *mut uv_shutdown_t, status: c_int) {
    let shutdown = ShutdownRequest::from_inner(req);
    if let Some(context) = shutdown.into_request().get_context::<ShutdownContext>() {
        let status = if status < 0 {
            Err(Errno::from_inner(status))
        } else {
            Ok(())
        };

        if let Some(ref mut shutdown_cb) = context.shutdown_cb {
            shutdown_cb.0(shutdown, status);
        }
    }
    shutdown.into_request().drop_context();
    shutdown.drop_request();
}

// impl

impl ShutdownRequest {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_shutdown_t>();
        let raw = unsafe { alloc(layout) as *mut uv_shutdown_t };
        if raw.is_null() {
            return Err(Errno::ENOMEM);
        }

        super::init_request(raw as *mut uv_req_t);

        Ok(Self { raw })
    }
    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        let request = self.into_request();
        if let Some(context) = unsafe { request.get_context::<ShutdownContext>() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "WriteRequest::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
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
        let mut request = self.into_request();
        match unsafe { request.get_context::<ShutdownContext>() } {
            Some(context) => context.data = data,
            None => {
                request.set_context(ShutdownContext {
                    data,
                    shutdown_cb: None,
                });
            }
        }
    }
}

// trait

impl<'a> From<ShutdownContext<'a>> for super::RequestContext {
    fn from(value: ShutdownContext<'a>) -> Self {
        Self { data: value.data }
    }
}

impl<'a, Fn> From<Fn> for ShutdownCallback<'a>
where
    Fn: FnMut(ShutdownRequest, Result<(), Errno>) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for ShutdownCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _| ()))
    }
}

// from_inner/into_inner

impl FromInner<*mut uv_shutdown_t> for ShutdownRequest {
    fn from_inner(raw: *mut uv_shutdown_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_shutdown_t> for ShutdownRequest {
    fn into_inner(self) -> *mut uv_shutdown_t {
        self.raw
    }
}
