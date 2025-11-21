pub(crate) mod write;
use std::{marker::PhantomData, os::raw::c_void, ptr::null_mut};

pub(crate) use write::*;

use crate::{
    inners::{FromInner, IntoInner},
    uv::{uv_req_get_data, uv_req_set_data, uv_req_t},
};

#[derive(Debug, Clone, Copy)]
pub struct Request<T> {
    raw: *mut uv_req_t,
    _marker: PhantomData<T>,
}

pub trait IRequest<T> {
    fn into_request(self) -> Request<T>;
}

pub(crate) fn init_request(raw: *mut uv_req_t) {
    unsafe { uv_req_set_data(raw, null_mut()) };
}

impl<C> Request<C> {
    pub(crate) fn from_raw(raw: *mut uv_req_t) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    pub fn get_context(&self) -> Option<&mut C> {
        let context = unsafe { uv_req_get_data(self.raw) };
        if context.is_null() {
            None
        } else {
            Some(unsafe { &mut *(context as *mut C) })
        }
    }

    pub fn set_context(&mut self, context: C) {
        unsafe { uv_req_set_data(self.raw, Box::into_raw(Box::new(context)) as *mut c_void) };
    }

    pub fn free_context(&mut self) {
        let context = unsafe { uv_req_get_data(self.raw) };
        if !context.is_null() {
            unsafe { drop(Box::from_raw(context)) }
        }
    }
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
