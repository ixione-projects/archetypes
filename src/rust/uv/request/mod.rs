pub(crate) mod write;
pub(crate) use write::*;

pub(crate) mod work;
pub(crate) use work::*;

pub(crate) mod fs;
pub(crate) use fs::*;

use std::{marker::PhantomData, os::raw::c_void, ptr::null_mut};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{uv_req_get_data, uv_req_set_data, uv_req_t},
};

#[derive(Debug, Clone, Copy)]
pub struct Request<T> {
    raw: *mut uv_req_t,
    _marker: PhantomData<T>,
}

pub trait IRequest<C>: Copy {
    fn into_request(self) -> Request<C>;

    fn get_context(&self) -> Option<&mut C> {
        let context = unsafe { uv_req_get_data(self.into_request().raw) };
        if context.is_null() {
            None
        } else {
            Some(unsafe { &mut *(context as *mut C) })
        }
    }

    fn set_context(&mut self, context: C) {
        unsafe {
            uv_req_set_data(
                self.into_request().raw,
                Box::into_raw(Box::new(context)) as *mut c_void,
            )
        };
    }

    fn free_context(&mut self) {
        let context = unsafe { uv_req_get_data(self.into_request().raw) };
        if !context.is_null() {
            unsafe { drop(Box::from_raw(context)) }
        }
    }
}

pub(crate) fn init_request(raw: *mut uv_req_t) {
    unsafe { uv_req_set_data(raw, null_mut()) };
}

impl<T> FromInner<*mut uv_req_t> for Request<T> {
    fn from_inner(raw: *mut uv_req_t) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }
}

impl<T> IntoInner<*mut uv_req_t> for Request<T> {
    fn into_inner(self) -> *mut uv_req_t {
        self.raw
    }
}
