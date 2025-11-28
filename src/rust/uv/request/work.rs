use std::{
    alloc::{Layout, alloc, dealloc},
    os::raw::c_int,
};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{Errno, IRequest, Loop, uv_queue_work, uv_req_t, uv_work_t},
};

impl<'a> super::IRequest<WorkContext<'a>> for WorkRequest {
    fn into_request(self) -> super::Request<WorkContext<'a>> {
        super::Request::from_inner(self.raw as *mut uv_req_t)
    }
}

pub struct WorkCallback<'a>(pub Box<dyn FnMut(&'a mut WorkRequest) + 'a>);
pub struct AfterWorkCallback<'a>(pub Box<dyn FnMut(WorkRequest, Result<(), Errno>) + 'a>);

pub struct WorkContext<'a> {
    pub(crate) work_cb: Option<WorkCallback<'a>>,
    pub(crate) after_work_cb: Option<AfterWorkCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkRequest {
    raw: *mut uv_work_t,
}

impl WorkRequest {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_work_t>();
        let raw = unsafe { alloc(layout) as *mut uv_work_t };
        super::init_request(raw as *mut uv_req_t);
        if raw.is_null() {
            Err(Errno::ENOMEM)
        } else {
            Ok(Self { raw })
        }
    }
}

impl Loop {
    pub fn queue_work<'a, WCB, AWCB>(
        &self,
        mut req: WorkRequest,
        work_cb: WCB,
        after_work_cb: AWCB,
    ) -> Result<(), Errno>
    where
        WCB: Into<WorkCallback<'a>>,
        AWCB: Into<AfterWorkCallback<'a>>,
    {
        match req.get_context() {
            Some(context) => {
                context.work_cb = Some(work_cb.into());
                context.after_work_cb = Some(after_work_cb.into());
            }
            None => {
                let new_context = WorkContext {
                    work_cb: Some(work_cb.into()),
                    after_work_cb: Some(after_work_cb.into()),
                };
                req.set_context(new_context);
            }
        };

        let result = unsafe {
            uv_queue_work(
                self.into_inner(),
                req.into_inner(),
                Some(uv_work_cb),
                Some(uv_after_work_cb),
            )
        };

        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }
}

pub(crate) unsafe extern "C" fn uv_work_cb(req: *mut uv_work_t) {
    let mut work = WorkRequest::from_inner(req);
    if let Some(context) = work.get_context() {
        if let Some(mut work_cb) = context.work_cb.take() {
            work_cb.0(&mut work);
        }
    }
}

pub(crate) unsafe extern "C" fn uv_after_work_cb(req: *mut uv_work_t, status: c_int) {
    let mut work = WorkRequest::from_inner(req);
    if let Some(context) = work.get_context() {
        let status = if status < 0 {
            Err(Errno::from_inner(status))
        } else {
            Ok(())
        };

        if let Some(mut after_work_cb) = context.after_work_cb.take() {
            after_work_cb.0(work, status);
        }
    }
    work.free_context();
    let layout = Layout::new::<uv_work_t>();
    unsafe { dealloc(req as *mut u8, layout) };
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

impl FromInner<*mut uv_work_t> for WorkRequest {
    fn from_inner(raw: *mut uv_work_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_work_t> for &WorkRequest {
    fn into_inner(self) -> *mut uv_work_t {
        self.raw
    }
}
