pub(crate) mod write;
use std::{marker::PhantomData, os::raw::c_void};

pub(crate) use write::*;

use crate::{
    inners::{FromInner, IntoInner},
    uv::{uv_req_get_data, uv_req_set_data, uv_req_t},
};

pub struct Request<T> {
    raw: *mut uv_req_t,
    _marker: PhantomData<T>,
}

pub trait IRequest<T> {
    fn into_request(&self) -> Request<T>;
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

impl<T> Request<T> {
    pub(crate) fn from_raw(raw: *mut uv_req_t) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    pub fn get_context(&self) -> Option<Box<T>> {
        let context = unsafe { uv_req_get_data(self.raw) };
        if context.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(context as *mut T) })
        }
    }

    pub fn set_context(&self, context: Box<T>) {
        unsafe { uv_req_set_data(self.raw, Box::into_raw(context) as *mut c_void) };
    }
}
