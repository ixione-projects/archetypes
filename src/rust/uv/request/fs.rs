use std::alloc::{Layout, alloc, dealloc};

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        Buf, ConstBuf, Errno, IRequest, Loop, MutBuf, buf, uv_fs_read, uv_fs_t, uv_fs_write,
        uv_req_t,
    },
};

impl<'a> super::IRequest<FileSystemContext<'a>> for FileSystemRequest {
    fn into_request(self) -> super::Request<FileSystemContext<'a>> {
        super::Request::from_inner(self.raw as *mut uv_req_t)
    }
}

pub struct FileSystemCallback<'a>(pub Box<dyn FnMut(FileSystemRequest) + 'a>);

pub struct FileSystemContext<'a> {
    fs_cb: Option<FileSystemCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct FileSystemRequest {
    raw: *mut uv_fs_t,
}

impl FileSystemRequest {
    pub fn new() -> Result<Self, Errno> {
        let layout = Layout::new::<uv_fs_t>();
        let raw = unsafe { alloc(layout) as *mut uv_fs_t };
        super::init_request(raw as *mut uv_req_t);
        if raw.is_null() {
            Err(Errno::ENOMEM)
        } else {
            Ok(Self { raw })
        }
    }

    pub fn result(&self) -> isize {
        unsafe { (*self.raw).result }
    }
}

// TODO: seperate into sync/async versions
impl Loop {
    pub fn fs_read<'a, B: Buf, FSCB>(
        &self,
        mut req: FileSystemRequest,
        file: i32,
        bufs: &[B],
        offset: i64,
        fs_cb: FSCB,
    ) -> Result<(), Errno>
    where
        FSCB: Into<FileSystemCallback<'a>>,
    {
        match req.get_context() {
            Some(context) => {
                context.fs_cb = Some(fs_cb.into());
            }
            None => {
                let new_context = FileSystemContext {
                    fs_cb: Some(fs_cb.into()),
                };
                req.set_context(new_context);
            }
        };

        let bigbuf = bufs.into_inner();
        let result = unsafe {
            uv_fs_read(
                self.into_inner(),
                req.into_inner(),
                file,
                bigbuf.as_ptr(),
                bigbuf.len() as u32,
                offset,
                Some(uv_fs_cb),
            )
        };

        // Box::leak(bigbuf); // FIXME: // do we need to leak the buf?
        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    pub fn fs_write<'a, B: Buf, FSCB>(
        &self,
        mut req: FileSystemRequest,
        file: i32,
        bufs: &[B],
        offset: i64,
        fs_cb: FSCB,
    ) -> Result<(), Errno>
    where
        FSCB: Into<FileSystemCallback<'a>>,
    {
        match req.get_context() {
            Some(context) => {
                context.fs_cb = Some(fs_cb.into());
            }
            None => {
                let new_context = FileSystemContext {
                    fs_cb: Some(fs_cb.into()),
                };
                req.set_context(new_context);
            }
        };

        let bigbuf = bufs.into_inner();
        let result = unsafe {
            uv_fs_write(
                self.into_inner(),
                req.into_inner(),
                file,
                bigbuf.as_ptr(),
                bigbuf.len() as u32,
                offset,
                Some(uv_fs_cb),
            )
        };

        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }
}

pub(crate) unsafe extern "C" fn uv_fs_cb(req: *mut uv_fs_t) {
    let mut fs = FileSystemRequest::from_inner(req);
    if let Some(context) = fs.get_context() {
        if let Some(mut fs_cb) = context.fs_cb.take() {
            fs_cb.0(fs);
        }
    }
    fs.free_context();
    let layout = Layout::new::<uv_fs_t>();
    unsafe { dealloc(req as *mut u8, layout) };
}

impl<'a, Fn> From<Fn> for FileSystemCallback<'a>
where
    Fn: FnMut(FileSystemRequest) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for FileSystemCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_| ()))
    }
}

impl FromInner<*mut uv_fs_t> for FileSystemRequest {
    fn from_inner(raw: *mut uv_fs_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_fs_t> for &FileSystemRequest {
    fn into_inner(self) -> *mut uv_fs_t {
        self.raw
    }
}
