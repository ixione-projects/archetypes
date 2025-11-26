use std::alloc::{Layout, alloc, dealloc};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        AllocCallback, CloseCallback, Errno, IHandle, Loop, uv_check_init, uv_check_start,
        uv_check_stop, uv_check_t, uv_handle_t,
    },
};

impl<'a> super::IHandleContext<'a> for CheckContext<'a> {
    fn into_handle_context(self) -> super::HandleContext<'a> {
        super::HandleContext::from(self)
    }
}

impl super::IHandle for CheckHandle {
    fn into_handle(self) -> super::Handle {
        super::Handle::from_inner(self.raw as *mut uv_handle_t)
    }

    fn free_handle(self) {
        let layout = Layout::new::<uv_check_t>();
        unsafe { dealloc(self.raw as *mut u8, layout) };
    }
}

pub struct CheckCallback<'a>(pub Box<dyn FnMut(&'a CheckHandle) + 'a>);

pub struct CheckContext<'a> {
    alloc_cb: Option<AllocCallback<'a>>,
    close_cb: Option<CloseCallback<'a>>,
    check_cb: Option<CheckCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct CheckHandle {
    raw: *mut uv_check_t,
}

impl CheckHandle {
    fn new(r#loop: &Loop) -> Result<Self, Errno> {
        let layout = Layout::new::<uv_check_t>();
        let raw = unsafe { alloc(layout) as *mut uv_check_t };
        super::init_handle(raw as *mut uv_handle_t);
        if raw.is_null() {
            return Err(Errno::ENOMEM);
        }

        let result = unsafe { uv_check_init(r#loop.into_inner(), raw) };
        if result < 0 {
            unsafe { dealloc(raw as *mut u8, layout) };
            return Err(Errno::from_inner(result));
        }
        Ok(Self { raw })
    }

    pub fn start<'a, CCB>(&mut self, check_cb: CCB) -> Result<(), Errno>
    where
        CCB: Into<CheckCallback<'a>>,
    {
        match self.get_context::<CheckContext>() {
            Some(ref mut context) => {
                context.check_cb = Some(check_cb.into());
            }
            None => {
                let new_context = CheckContext {
                    alloc_cb: None,
                    close_cb: None,
                    check_cb: Some(check_cb.into()),
                };
                self.set_context(new_context);
            }
        };

        let result = unsafe { uv_check_start(self.raw, Some(uv_check_cb)) };
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    pub fn stop(&mut self) {
        unsafe { uv_check_stop(self.raw) };
    }
}

impl Loop {
    pub fn new_check(&self) -> Result<CheckHandle, Errno> {
        return CheckHandle::new(self);
    }
}

pub(crate) unsafe extern "C" fn uv_check_cb(handle: *mut uv_check_t) {
    let handle = CheckHandle::from_inner(handle);
    if let Some(context) = handle.get_context::<CheckContext>() {
        if let Some(ref mut check_cb) = context.check_cb {
            check_cb.0(&handle);
        }
    }
}

impl<'a, Fn> From<Fn> for CheckCallback<'a>
where
    Fn: FnMut(&CheckHandle) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for CheckCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_| ()))
    }
}

impl<'a> From<CheckContext<'a>> for super::HandleContext<'a> {
    fn from(value: CheckContext<'a>) -> Self {
        Self {
            alloc_cb: value.alloc_cb,
            close_cb: value.close_cb,
        }
    }
}

impl FromInner<*mut uv_check_t> for CheckHandle {
    fn from_inner(raw: *mut uv_check_t) -> Self {
        Self { raw }
    }
}
