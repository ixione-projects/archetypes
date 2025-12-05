use std::{
    alloc::{Layout, alloc, dealloc},
    any::{Any, TypeId},
    os::raw::{c_int, c_void},
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, IRequest, uv_req_t, uv_write_t},
};

// super

impl<'a> super::IRequestContext for WriteContext<'a> {
    fn into_request_context(self) -> super::RequestContext {
        super::RequestContext::from(self)
    }
}

impl<'a> super::IRequest for WriteRequest {
    fn into_request(self) -> super::Request {
        super::Request::from_inner(self.raw as *mut uv_req_t)
    }

    fn drop_request(self) {
        let layout = Layout::new::<uv_write_t>();
        unsafe { dealloc(self.raw as *mut u8, layout) };
    }
}

// type

pub struct WriteCallback<'a>(pub Box<dyn FnMut(WriteRequest, Result<(), Errno>) + 'a>);

#[repr(C)]
pub struct WriteContext<'a> {
    pub(crate) data: *mut c_void,
    pub(crate) write_cb: Option<WriteCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct WriteRequest {
    raw: *mut uv_write_t,
}

// fn

pub(crate) unsafe extern "C" fn uv_write_cb(req: *mut uv_write_t, status: c_int) {
    let write = WriteRequest::from_inner(req);
    if let Some(context) = write.into_request().get_context::<WriteContext>() {
        let status = if status < 0 {
            Err(Errno::from_inner(status))
        } else {
            Ok(())
        };

        if let Some(ref mut write_cb) = context.write_cb {
            write_cb.0(write, status);
        }
    }
    write.into_request().drop_context();
    write.drop_request();
}

// impl

impl WriteRequest {
    pub fn new() -> Self {
        let layout = Layout::new::<uv_write_t>();
        let raw = unsafe { alloc(layout) as *mut uv_write_t };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        super::init_request(raw as *mut uv_req_t);

        Self { raw }
    }
    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        let request = self.into_request();
        if let Some(context) = unsafe { request.get_context::<WriteContext>() } {
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
        match unsafe { request.get_context::<WriteContext>() } {
            Some(context) => context.data = data,
            None => {
                request.set_context(WriteContext {
                    data,
                    write_cb: None,
                });
            }
        }
    }
}

// trait

impl<'a> From<WriteContext<'a>> for super::RequestContext {
    fn from(value: WriteContext<'a>) -> Self {
        Self { data: value.data }
    }
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

// inner

impl FromInner<*mut uv_write_t> for WriteRequest {
    fn from_inner(raw: *mut uv_write_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_write_t> for WriteRequest {
    fn into_inner(self) -> *mut uv_write_t {
        self.raw
    }
}
