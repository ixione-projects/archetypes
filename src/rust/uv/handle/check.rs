use std::{
    alloc::{Layout, alloc, dealloc},
    any::{Any, TypeId},
    os::raw::c_void,
    ptr::null_mut,
};

use crate::{
    inners::{FromInner, IntoInner},
    result,
    uv::{
        AllocCallback, CloseCallback, Errno, IHandle, Loop, uv_check_init, uv_check_start,
        uv_check_stop, uv_check_t, uv_handle_t,
    },
};

// super

impl<'a> super::IHandleContext<'a> for CheckContext<'a> {
    fn into_handle_context(self) -> super::HandleContext<'a> {
        super::HandleContext::from(self)
    }
}

impl super::IHandle for CheckHandle {
    fn into_handle(self) -> super::Handle {
        super::Handle::from_inner(self.raw as *mut uv_handle_t)
    }

    fn drop_handle(self) {
        let layout = Layout::new::<uv_check_t>();
        unsafe { dealloc(self.raw as *mut u8, layout) };
    }
}

// type

pub struct CheckCallback<'a>(pub Box<dyn FnMut(&'a CheckHandle) + 'a>);

#[repr(C)]
pub struct CheckContext<'a> {
    alloc_cb: Option<AllocCallback<'a>>,
    close_cb: Option<CloseCallback<'a>>,
    data: *mut c_void,
    check_cb: Option<CheckCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct CheckHandle {
    raw: *mut uv_check_t,
}

// fn

pub(crate) unsafe extern "C" fn uv_check_cb(handle: *mut uv_check_t) {
    let handle = CheckHandle::from_inner(handle);
    if let Some(context) = handle.into_handle().get_context::<CheckContext>() {
        if let Some(ref mut check_cb) = context.check_cb {
            check_cb.0(&handle);
        }
    }
}

// impl

impl CheckHandle {
    fn new(r#loop: &Loop) -> Result<Self, Errno> {
        let layout = Layout::new::<uv_check_t>();
        let raw = unsafe { alloc(layout) as *mut uv_check_t };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        super::init_handle(raw as *mut uv_handle_t);

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
        let mut handle = self.into_handle();
        match unsafe { handle.get_context::<CheckContext>() } {
            Some(ref mut context) => {
                context.check_cb = Some(check_cb.into());
            }
            None => {
                handle.set_context(CheckContext {
                    alloc_cb: None,
                    close_cb: None,
                    data: null_mut(),
                    check_cb: Some(check_cb.into()),
                });
            }
        };

        result!(unsafe { uv_check_start(self.raw, Some(uv_check_cb)) })
    }

    pub fn stop(&mut self) {
        unsafe { uv_check_stop(self.raw) };
    }

    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        let handle = self.into_handle();
        if let Some(context) = unsafe { handle.get_context::<CheckContext>() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "CheckHandle::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
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
        let mut handle = self.into_handle();
        match unsafe { handle.get_context::<CheckContext>() } {
            Some(context) => context.data = data,
            None => {
                handle.set_context(CheckContext {
                    alloc_cb: None,
                    close_cb: None,
                    data,
                    check_cb: None,
                });
            }
        }
    }
}

impl Loop {
    pub fn new_check(&self) -> Result<CheckHandle, Errno> {
        return CheckHandle::new(self);
    }
}

// trait

impl<'a> From<CheckContext<'a>> for super::HandleContext<'a> {
    fn from(value: CheckContext<'a>) -> Self {
        Self {
            alloc_cb: value.alloc_cb,
            close_cb: value.close_cb,
            data: value.data,
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

// inner

impl FromInner<*mut uv_check_t> for CheckHandle {
    fn from_inner(raw: *mut uv_check_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_check_t> for CheckHandle {
    fn into_inner(self) -> *mut uv_check_t {
        self.raw
    }
}
