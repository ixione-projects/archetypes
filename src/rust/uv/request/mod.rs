// mod

pub(crate) mod write;
pub(crate) use write::*;

pub(crate) mod shutdown;
pub(crate) use shutdown::*;

pub(crate) mod fs;
pub(crate) use fs::*;

pub(crate) mod work;
pub(crate) use work::*;

use std::{
    any::{Any, TypeId},
    ffi::CStr,
    os::raw::c_void,
    ptr::null_mut,
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        self, Errno, uv_cancel, uv_fs_t, uv_req_get_data, uv_req_get_type, uv_req_set_data,
        uv_req_t, uv_req_type, uv_req_type_name, uv_work_t, uv_write_t,
    },
};

// type

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestType {
    UNKNOWN_REQ,
    REQ,
    CONNECT,
    WRITE,
    SHUTDOWN,
    UDP_SEND,
    FS,
    WORK,
    GETADDRINFO,
    GETNAMEINFO,
    RANDOM,
    REQ_TYPE_MAX,
}

#[repr(C)]
pub struct RequestContext {
    data: *mut c_void,
}

pub trait IRequestContext {
    fn into_request_context(self) -> RequestContext;
}

#[derive(Debug, Clone, Copy)]
pub struct Request {
    raw: *mut uv_req_t,
}

pub trait IRequest: Copy {
    fn into_request(self) -> Request;

    fn drop_request(self);

    fn cancel(self) -> Result<(), Errno> {
        self.into_request().cancel()
    }

    fn get_type(&self) -> RequestType {
        self.into_request().get_type()
    }
}

// fn

pub(crate) fn init_request(raw: *mut uv_req_t) {
    unsafe { uv_req_set_data(raw, null_mut()) };
}

// impl

impl RequestType {
    pub fn name(&self) -> String {
        unsafe { CStr::from_ptr(uv_req_type_name(self.into_inner())) }
            .to_string_lossy()
            .into_owned()
    }
}

impl IRequestContext for RequestContext {
    fn into_request_context(self) -> RequestContext {
        self
    }
}

impl Request {
    pub fn cancel(mut self) -> Result<(), Errno> {
        let result = unsafe { uv_cancel(self.raw) };
        if result < 0 {
            return Err(Errno::from_inner(result));
        }

        self.drop_context();
        self.drop_request();

        Ok(())
    }

    pub fn get_type(&self) -> RequestType {
        RequestType::from_inner(unsafe { uv_req_get_type(self.raw) })
    }

    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        if let Some(context) = unsafe { self.get_context::<RequestContext>() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "Request::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
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
        match unsafe { self.get_context::<RequestContext>() } {
            Some(context) => context.data = data,
            None => {
                self.set_context(RequestContext { data });
            }
        }
    }

    pub(crate) unsafe fn get_context<C: IRequestContext>(&self) -> Option<&mut C> {
        let context = uv_req_get_data(self.raw);
        if context.is_null() {
            None
        } else {
            Some(&mut *(context as *mut C))
        }
    }

    pub(crate) fn set_context<C: IRequestContext>(&mut self, context: C) {
        unsafe { uv_req_set_data(self.raw, Box::into_raw(Box::new(context)) as *mut c_void) };
    }

    pub(crate) fn drop_context(&mut self) {
        let context = unsafe { uv_req_get_data(self.raw) };
        if !context.is_null() {
            drop(unsafe { Box::from_raw(context) })
        }
    }
}

impl IRequest for Request {
    fn into_request(self) -> Request {
        todo!()
    }

    fn drop_request(self) {
        match self.get_type() {
            RequestType::WRITE => {
                WriteRequest::from_inner(self.raw as *mut uv_write_t).drop_request()
            }
            RequestType::FS => {
                FileSystemRequest::from_inner(self.raw as *mut uv_fs_t).drop_request()
            }
            RequestType::WORK => WorkRequest::from_inner(self.raw as *mut uv_work_t).drop_request(),
            _ => panic!(
                "Request::drop_request: unexpected type [{}]",
                self.get_type().name()
            ),
        };
    }
}

// from_inner/into_inner

impl FromInner<uv_req_type> for RequestType {
    fn from_inner(value: uv_req_type) -> Self {
        match value {
            uv::uv_req_type_UV_UNKNOWN_REQ => RequestType::UNKNOWN_REQ,
            uv::uv_req_type_UV_REQ => RequestType::REQ,
            uv::uv_req_type_UV_CONNECT => RequestType::CONNECT,
            uv::uv_req_type_UV_WRITE => RequestType::WRITE,
            uv::uv_req_type_UV_SHUTDOWN => RequestType::SHUTDOWN,
            uv::uv_req_type_UV_UDP_SEND => RequestType::UDP_SEND,
            uv::uv_req_type_UV_FS => RequestType::FS,
            uv::uv_req_type_UV_WORK => RequestType::WORK,
            uv::uv_req_type_UV_GETADDRINFO => RequestType::GETADDRINFO,
            uv::uv_req_type_UV_GETNAMEINFO => RequestType::GETNAMEINFO,
            uv::uv_req_type_UV_RANDOM => RequestType::RANDOM,
            uv::uv_req_type_UV_REQ_TYPE_MAX => RequestType::REQ_TYPE_MAX,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<uv_req_type> for RequestType {
    fn into_inner(self) -> uv_req_type {
        match self {
            RequestType::UNKNOWN_REQ => uv::uv_req_type_UV_UNKNOWN_REQ,
            RequestType::REQ => uv::uv_req_type_UV_REQ,
            RequestType::CONNECT => uv::uv_req_type_UV_CONNECT,
            RequestType::WRITE => uv::uv_req_type_UV_WRITE,
            RequestType::SHUTDOWN => uv::uv_req_type_UV_SHUTDOWN,
            RequestType::UDP_SEND => uv::uv_req_type_UV_UDP_SEND,
            RequestType::FS => uv::uv_req_type_UV_FS,
            RequestType::WORK => uv::uv_req_type_UV_WORK,
            RequestType::GETADDRINFO => uv::uv_req_type_UV_GETADDRINFO,
            RequestType::GETNAMEINFO => uv::uv_req_type_UV_GETNAMEINFO,
            RequestType::RANDOM => uv::uv_req_type_UV_RANDOM,
            RequestType::REQ_TYPE_MAX => uv::uv_req_type_UV_REQ_TYPE_MAX,
        }
    }
}

impl FromInner<*mut uv_req_t> for Request {
    fn from_inner(raw: *mut uv_req_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_req_t> for Request {
    fn into_inner(self) -> *mut uv_req_t {
        self.raw
    }
}
