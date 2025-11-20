use std::{
    alloc::{Layout, alloc, dealloc},
    os::raw::c_int,
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, IRequest, uv_errno_t, uv_req_t, uv_write_t},
};

pub struct WriteCallback(pub Box<dyn FnMut(WriteRequest, Result<(), Errno>)>);

impl<Fn> From<Fn> for WriteCallback
where
    Fn: FnMut(WriteRequest, Result<(), Errno>) + 'static,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl From<()> for WriteCallback {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _| {}))
    }
}

pub struct WriteContext {
    pub(crate) cb: Option<WriteCallback>,
}

#[derive(Debug)]
pub struct WriteRequest {
    raw: *mut uv_write_t,
}

// impl Deref for WriteRequest {
//     type Target = WriteContext;
//
//     fn deref(&self) -> &Self::Target {}
// }

impl FromInner<*mut uv_write_t> for WriteRequest {
    fn from_inner(raw: *mut uv_write_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_write_t> for &WriteRequest {
    fn into_inner(self) -> *mut uv_write_t {
        self.raw
    }
}

impl WriteRequest {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_write_t>();
        let raw = unsafe { alloc(layout) as *mut uv_write_t };
        if raw.is_null() {
            return Err(Errno::ENOMEM);
        }

        let write = Self { raw };
        write
            .into_request()
            .set_context(Box::new(WriteContext { cb: None }));
        Ok(write)
    }
}

impl super::IRequest<WriteContext> for WriteRequest {
    fn into_request(&self) -> super::Request<WriteContext> {
        super::Request::from_raw(self.raw as *mut uv_req_t)
    }
}

pub(crate) unsafe extern "C" fn uv_write_cb(req: *mut uv_write_t, status: c_int) {
    let write = WriteRequest::from_inner(req);
    if let Some(mut context) = write.into_request().get_context() {
        let status = if status < 0 {
            Err(Errno::from_inner(status as uv_errno_t))
        } else {
            Ok(())
        };

        if let Some(mut cb) = context.cb.take() {
            cb.0(write, status);
        }
        drop(context);
    }
    let layout = Layout::new::<uv_write_t>();
    unsafe { dealloc(req as *mut u8, layout) };
}
