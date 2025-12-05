use std::{
    alloc::{Layout, alloc, dealloc},
    any::{Any, TypeId},
    os::raw::{c_int, c_void},
    ptr::null_mut,
};

use crate::{
    inners::{FromInner, IntoInner},
    result,
    uv::{Errno, IRequest, Loop, uv_queue_work, uv_req_t, uv_work_t},
};

// super

impl<'a> super::IRequestContext for WorkContext<'a> {
    fn into_request_context(self) -> super::RequestContext {
        super::RequestContext::from(self)
    }
}

impl<'a> super::IRequest for WorkRequest {
    fn into_request(self) -> super::Request {
        super::Request::from_inner(self.raw as *mut uv_req_t)
    }

    fn drop_request(self) {
        let layout = Layout::new::<uv_work_t>();
        unsafe { dealloc(self.raw as *mut u8, layout) };
    }
}

// type

pub struct WorkCallback<'a>(pub Box<dyn FnMut(&'a mut WorkRequest) + 'a>);
pub struct AfterWorkCallback<'a>(pub Box<dyn FnMut(WorkRequest, Result<(), Errno>) + 'a>);

#[repr(C)]
pub struct WorkContext<'a> {
    data: *mut c_void,
    work_cb: Option<WorkCallback<'a>>,
    after_work_cb: Option<AfterWorkCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkRequest {
    raw: *mut uv_work_t,
}

// fn

pub(crate) unsafe extern "C" fn uv_work_cb(req: *mut uv_work_t) {
    let mut work = WorkRequest::from_inner(req);
    if let Some(context) = work.into_request().get_context::<WorkContext>() {
        if let Some(ref mut work_cb) = context.work_cb {
            work_cb.0(&mut work);
        }
    }
}

pub(crate) unsafe extern "C" fn uv_after_work_cb(req: *mut uv_work_t, status: c_int) {
    let work = WorkRequest::from_inner(req);
    if let Some(context) = work.into_request().get_context::<WorkContext>() {
        let status = if status < 0 {
            Err(Errno::from_inner(status))
        } else {
            Ok(())
        };

        if let Some(ref mut after_work_cb) = context.after_work_cb {
            after_work_cb.0(work, status);
        }
    }
    work.into_request().drop_context();
    work.drop_request();
}

// impl

impl WorkRequest {
    pub fn new() -> Self {
        let layout = Layout::new::<uv_work_t>();
        let raw = unsafe { alloc(layout) as *mut uv_work_t };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        super::init_request(raw as *mut uv_req_t);

        Self { raw }
    }

    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        let request = self.into_request();
        if let Some(context) = unsafe { request.get_context::<WorkContext>() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "WorkRequest::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
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
        match unsafe { request.get_context::<WorkContext>() } {
            Some(context) => context.data = data,
            None => {
                request.set_context(WorkContext {
                    data,
                    work_cb: None,
                    after_work_cb: None,
                });
            }
        }
    }
}

impl Loop {
    pub fn queue_work<'a, WCB, AWCB>(
        &self,
        req: WorkRequest,
        work_cb: WCB,
        after_work_cb: AWCB,
    ) -> Result<(), Errno>
    where
        WCB: Into<WorkCallback<'a>>,
        AWCB: Into<AfterWorkCallback<'a>>,
    {
        let mut request = req.into_request();
        match unsafe { request.get_context::<WorkContext>() } {
            Some(context) => {
                context.work_cb = Some(work_cb.into());
                context.after_work_cb = Some(after_work_cb.into());
            }
            None => {
                let new_context = WorkContext {
                    data: null_mut(),
                    work_cb: Some(work_cb.into()),
                    after_work_cb: Some(after_work_cb.into()),
                };
                request.set_context(new_context);
            }
        };

        result!(unsafe {
            uv_queue_work(
                self.into_inner(),
                req.into_inner(),
                Some(uv_work_cb),
                Some(uv_after_work_cb),
            )
        })
    }
}

// trait

impl<'a> From<WorkContext<'a>> for super::RequestContext {
    fn from(value: WorkContext<'a>) -> Self {
        Self { data: value.data }
    }
}

impl<'a, Fn> From<Fn> for WorkCallback<'a>
where
    Fn: FnMut(&'a mut WorkRequest) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for WorkCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_| ()))
    }
}

impl<'a, Fn> From<Fn> for AfterWorkCallback<'a>
where
    Fn: FnMut(WorkRequest, Result<(), Errno>) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for AfterWorkCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _| ()))
    }
}

// inner

impl FromInner<*mut uv_work_t> for WorkRequest {
    fn from_inner(raw: *mut uv_work_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_work_t> for WorkRequest {
    fn into_inner(self) -> *mut uv_work_t {
        self.raw
    }
}
