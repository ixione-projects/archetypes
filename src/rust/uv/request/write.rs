use std::{
    alloc::{Layout, alloc, dealloc},
    os::raw::c_int,
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, IRequest, uv_req_t, uv_write_t},
};

impl<'a> super::IRequest<WriteContext<'a>> for WriteRequest {
    fn into_request(self) -> super::Request<WriteContext<'a>> {
        super::Request::from_inner(self.raw as *mut uv_req_t)
    }
}

pub struct WriteCallback<'a>(pub Box<dyn FnMut(WriteRequest, Result<(), Errno>) + 'a>);

pub struct WriteContext<'a> {
    pub(crate) write_cb: Option<WriteCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct WriteRequest {
    raw: *mut uv_write_t,
}

impl WriteRequest {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_write_t>();
        let raw = unsafe { alloc(layout) as *mut uv_write_t };
        super::init_request(raw as *mut uv_req_t);
        if raw.is_null() {
            Err(Errno::ENOMEM)
        } else {
            Ok(Self { raw })
        }
    }
}

pub(crate) unsafe extern "C" fn uv_write_cb(req: *mut uv_write_t, status: c_int) {
    let mut write = WriteRequest::from_inner(req);
    if let Some(context) = write.get_context() {
        let status = if status < 0 {
            Err(Errno::from_inner(status))
        } else {
            Ok(())
        };

        if let Some(mut write_cb) = context.write_cb.take() {
            write_cb.0(write, status);
        }
    }
    write.free_context();
    let layout = Layout::new::<uv_write_t>();
    unsafe { dealloc(req as *mut u8, layout) };
}

impl<'a, Fn> From<Fn> for WriteCallback<'a>
where
    Fn: FnMut(WriteRequest, Result<(), Errno>) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for WriteCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _| ()))
    }
}

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
