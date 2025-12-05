use std::{
    alloc::{Layout, alloc, dealloc},
    ffi::CString,
    os::raw::c_void,
    path::Path,
    ptr::null_mut,
};

use crate::{
    inners::{FromInner, IntoInner},
    result,
    uv::{
        self, Buf, Errno, IRequest, Loop, uv_buf_t, uv_fs_close, uv_fs_get_result, uv_fs_open,
        uv_fs_read, uv_fs_req_cleanup, uv_fs_t, uv_fs_type, uv_fs_write, uv_req_t,
    },
};

// super

impl<'a> super::IRequestContext for FileSystemContext<'a> {
    fn into_request_context(self) -> super::RequestContext {
        super::RequestContext::from(self)
    }
}

impl<'a> super::IRequest for FileSystemRequest {
    fn into_request(self) -> super::Request {
        super::Request::from_inner(self.raw as *mut uv_req_t)
    }

    fn drop_request(self) {
        let layout = Layout::new::<uv_fs_t>();
        unsafe { dealloc(self.raw as *mut u8, layout) };
    }
}

// type

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSystemRequestType {
    UNKNOWN,
    CUSTOM,
    OPEN,
    CLOSE,
    READ,
    WRITE,
    SENDFILE,
    STAT,
    LSTAT,
    FSTAT,
    FTRUNCATE,
    UTIME,
    FUTIME,
    ACCESS,
    CHMOD,
    FCHMOD,
    FSYNC,
    FDATASYNC,
    UNLINK,
    RMDIR,
    MKDIR,
    MKDTEMP,
    RENAME,
    SCANDIR,
    LINK,
    SYMLINK,
    READLINK,
    CHOWN,
    FCHOWN,
    REALPATH,
    COPYFILE,
    LCHOWN,
    OPENDIR,
    READDIR,
    CLOSEDIR,
    STATFS,
    MKSTEMP,
    LUTIME,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    RDONLY,
    WRONLY,
    RDWR,
}

pub type OpenOption = u32;

pub const APPEND: OpenOption = 1024;
pub const CREAT: OpenOption = 64;
pub const DIRECT: OpenOption = 16384;
pub const DIRECTORY: OpenOption = 65536;
pub const DSYNC: OpenOption = 4096;
pub const EXCL: OpenOption = 128;
pub const EXLOCK: OpenOption = 0;
pub const NOATIME: OpenOption = 0;
pub const NOCTTY: OpenOption = 256;
pub const NOFOLLOW: OpenOption = 131072;
pub const NONBLOCK: OpenOption = 2048;
pub const SYMLINK: OpenOption = 0;
pub const SYNC: OpenOption = 1052672;
pub const TRUNC: OpenOption = 512;
pub const FILEMAP: OpenOption = 0;
pub const RANDOM: OpenOption = 0;
pub const SHORT_LIVED: OpenOption = 0;
pub const SEQUENTIAL: OpenOption = 0;
pub const TEMPORARY: OpenOption = 0;

pub struct OpenOptionSet(u32);

pub struct FileSystemCallback<'a>(pub Box<dyn FnMut(FileSystemRequest) + 'a>);

#[repr(C)]
pub struct FileSystemContext<'a> {
    data: *mut c_void,
    fs_cb: Option<FileSystemCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct FileSystemRequest {
    raw: *mut uv_fs_t,
}

// fn

pub(crate) unsafe extern "C" fn uv_fs_cb(req: *mut uv_fs_t) {
    let fs = FileSystemRequest::from_inner(req);
    if let Some(context) = fs.into_request().get_context::<FileSystemContext>() {
        if let Some(ref mut fs_cb) = context.fs_cb {
            fs_cb.0(fs);
        }
    }
    // TODO: add cleanup to all request types
    fs.cleanup();
    fs.into_request().drop_context();
    fs.drop_request();
}

// impl

impl OpenOptionSet {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn set(&mut self, option: OpenOption) -> &mut Self {
        self.0 |= option;
        self
    }

    pub fn unset(&mut self, option: OpenOption) -> &mut Self {
        self.0 &= !option;
        self
    }

    pub fn has(&self, option: OpenOption) -> bool {
        (self.0 & option) != 0
    }
}

impl FileSystemRequest {
    pub fn new() -> Self {
        let layout = Layout::new::<uv_fs_t>();
        let raw = unsafe { alloc(layout) as *mut uv_fs_t };
        if raw.is_null() {
            panic!("{}", Errno::ENOMEM);
        }

        super::init_request(raw as *mut uv_req_t);

        Self { raw }
    }

    pub fn cleanup(&self) {
        unsafe { uv_fs_req_cleanup(self.raw) };
    }

    pub fn result(&self) -> isize {
        unsafe { uv_fs_get_result(self.raw) }
    }
}

impl Loop {
    pub fn fs_close<'a, FSCB>(
        &self,
        req: FileSystemRequest,
        file: i32,
        fs_cb: FSCB,
    ) -> Result<(), Errno>
    where
        FSCB: Into<FileSystemCallback<'a>>,
    {
        let mut request = req.into_request();
        match unsafe { request.get_context::<FileSystemContext>() } {
            Some(context) => {
                context.fs_cb = Some(fs_cb.into());
            }
            None => {
                request.set_context(FileSystemContext {
                    data: null_mut(),
                    fs_cb: Some(fs_cb.into()),
                });
            }
        };

        result!(unsafe { uv_fs_close(self.into_inner(), req.into_inner(), file, Some(uv_fs_cb)) })
    }

    pub fn fs_close_sync(&self, req: FileSystemRequest, file: i32) -> Result<(), Errno> {
        result!(unsafe { uv_fs_close(self.into_inner(), req.into_inner(), file, None) })
    }

    pub fn fs_open<'a, FSCB>(
        &self,
        req: FileSystemRequest,
        path: &Path,
        flags: OpenOptionSet,
        mode: OpenMode,
        fs_cb: FSCB,
    ) -> Result<(), Errno>
    where
        FSCB: Into<FileSystemCallback<'a>>,
    {
        let mut request = req.into_request();
        match unsafe { request.get_context::<FileSystemContext>() } {
            Some(context) => {
                context.fs_cb = Some(fs_cb.into());
            }
            None => {
                request.set_context(FileSystemContext {
                    data: null_mut(),
                    fs_cb: Some(fs_cb.into()),
                });
            }
        };

        match CString::new(path.as_os_str().as_encoded_bytes()) {
            Ok(path) => {
                result!(unsafe {
                    uv_fs_open(
                        self.into_inner(),
                        req.into_inner(),
                        path.as_ptr() as *const i8,
                        flags.0 as i32,
                        mode.into_inner() as i32,
                        Some(uv_fs_cb),
                    )
                })
            }
            Err(_) => Err(Errno::EINVAL),
        }
    }

    // TODO: need a better type for fd
    pub fn fs_open_sync(
        &self,
        req: FileSystemRequest,
        path: &Path,
        flags: OpenOptionSet,
        mode: OpenMode,
    ) -> Result<i32, Errno> {
        match CString::new(path.as_os_str().as_encoded_bytes()) {
            Ok(path) => {
                let result = unsafe {
                    uv_fs_open(
                        self.into_inner(),
                        req.into_inner(),
                        path.as_ptr() as *const i8,
                        flags.0 as i32,
                        mode.into_inner() as i32,
                        Some(uv_fs_cb),
                    )
                };

                if result < 0 {
                    Err(Errno::from_inner(result))
                } else {
                    Ok(result)
                }
            }
            Err(_) => Err(Errno::EINVAL),
        }
    }

    pub fn fs_read<'a, FSCB>(
        &self,
        req: FileSystemRequest,
        file: i32,
        bufs: &[Buf],
        offset: i64,
        fs_cb: FSCB,
    ) -> Result<(), Errno>
    where
        FSCB: Into<FileSystemCallback<'a>>,
    {
        let mut request = req.into_request();
        match unsafe { request.get_context::<FileSystemContext>() } {
            Some(context) => {
                context.fs_cb = Some(fs_cb.into());
            }
            None => {
                request.set_context(FileSystemContext {
                    data: null_mut(),
                    fs_cb: Some(fs_cb.into()),
                });
            }
        };

        let (bufs, nbufs) = bufs.into_inner();
        result!(unsafe {
            uv_fs_read(
                self.into_inner(),
                req.into_inner(),
                file,
                bufs,
                nbufs as u32,
                offset,
                Some(uv_fs_cb),
            )
        })
    }

    pub fn fs_read_sync(
        &self,
        req: FileSystemRequest,
        file: i32,
        bufs: &[Buf],
        offset: i64,
    ) -> Result<(&[Buf], isize), Errno> {
        let (bufs, nbufs) = bufs.into_inner();
        let result = unsafe {
            uv_fs_read(
                self.into_inner(),
                req.into_inner(),
                file,
                bufs,
                nbufs as u32,
                offset,
                None,
            )
        };

        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            let ret = req.result();
            req.cleanup();
            req.into_request().drop_context();
            req.drop_request();
            Ok((
                FromInner::<(*mut uv_buf_t, usize)>::from_inner((bufs, nbufs)),
                ret,
            ))
        }
    }

    pub fn fs_write<'a, FSCB>(
        &self,
        req: FileSystemRequest,
        file: i32,
        bufs: &[Buf],
        offset: i64,
        fs_cb: FSCB,
    ) -> Result<(), Errno>
    where
        FSCB: Into<FileSystemCallback<'a>>,
    {
        let mut request = req.into_request();
        match unsafe { request.get_context::<FileSystemContext>() } {
            Some(context) => {
                context.fs_cb = Some(fs_cb.into());
            }
            None => {
                request.set_context(FileSystemContext {
                    data: null_mut(),
                    fs_cb: Some(fs_cb.into()),
                });
            }
        };

        let (bufs, nbufs) = bufs.into_inner();
        result!(unsafe {
            uv_fs_write(
                self.into_inner(),
                req.into_inner(),
                file,
                bufs,
                nbufs as u32,
                offset,
                Some(uv_fs_cb),
            )
        })
    }

    pub fn fs_write_sync(
        &self,
        req: FileSystemRequest,
        file: i32,
        bufs: &[Buf],
        offset: i64,
    ) -> Result<isize, Errno> {
        let (bufs, nbufs) = bufs.into_inner();
        let result = unsafe {
            uv_fs_write(
                self.into_inner(),
                req.into_inner(),
                file,
                bufs,
                nbufs as u32,
                offset,
                Some(uv_fs_cb),
            )
        };

        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            let ret = req.result();
            req.cleanup();
            req.into_request().drop_context();
            req.drop_request();
            Ok(ret)
        }
    }
}

// trait

impl<'a> From<FileSystemContext<'a>> for super::RequestContext {
    fn from(value: FileSystemContext<'a>) -> Self {
        Self { data: value.data }
    }
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

// inner

impl FromInner<uv_fs_type> for FileSystemRequestType {
    fn from_inner(value: uv_fs_type) -> Self {
        match value {
            uv::uv_fs_type_UV_FS_UNKNOWN => FileSystemRequestType::UNKNOWN,
            uv::uv_fs_type_UV_FS_CUSTOM => FileSystemRequestType::CUSTOM,
            uv::uv_fs_type_UV_FS_OPEN => FileSystemRequestType::OPEN,
            uv::uv_fs_type_UV_FS_CLOSE => FileSystemRequestType::CLOSE,
            uv::uv_fs_type_UV_FS_READ => FileSystemRequestType::READ,
            uv::uv_fs_type_UV_FS_WRITE => FileSystemRequestType::WRITE,
            uv::uv_fs_type_UV_FS_SENDFILE => FileSystemRequestType::SENDFILE,
            uv::uv_fs_type_UV_FS_STAT => FileSystemRequestType::STAT,
            uv::uv_fs_type_UV_FS_LSTAT => FileSystemRequestType::LSTAT,
            uv::uv_fs_type_UV_FS_FSTAT => FileSystemRequestType::FSTAT,
            uv::uv_fs_type_UV_FS_FTRUNCATE => FileSystemRequestType::FTRUNCATE,
            uv::uv_fs_type_UV_FS_UTIME => FileSystemRequestType::UTIME,
            uv::uv_fs_type_UV_FS_FUTIME => FileSystemRequestType::FUTIME,
            uv::uv_fs_type_UV_FS_ACCESS => FileSystemRequestType::ACCESS,
            uv::uv_fs_type_UV_FS_CHMOD => FileSystemRequestType::CHMOD,
            uv::uv_fs_type_UV_FS_FCHMOD => FileSystemRequestType::FCHMOD,
            uv::uv_fs_type_UV_FS_FSYNC => FileSystemRequestType::FSYNC,
            uv::uv_fs_type_UV_FS_FDATASYNC => FileSystemRequestType::FDATASYNC,
            uv::uv_fs_type_UV_FS_UNLINK => FileSystemRequestType::UNLINK,
            uv::uv_fs_type_UV_FS_RMDIR => FileSystemRequestType::RMDIR,
            uv::uv_fs_type_UV_FS_MKDIR => FileSystemRequestType::MKDIR,
            uv::uv_fs_type_UV_FS_MKDTEMP => FileSystemRequestType::MKDTEMP,
            uv::uv_fs_type_UV_FS_RENAME => FileSystemRequestType::RENAME,
            uv::uv_fs_type_UV_FS_SCANDIR => FileSystemRequestType::SCANDIR,
            uv::uv_fs_type_UV_FS_LINK => FileSystemRequestType::LINK,
            uv::uv_fs_type_UV_FS_SYMLINK => FileSystemRequestType::SYMLINK,
            uv::uv_fs_type_UV_FS_READLINK => FileSystemRequestType::READLINK,
            uv::uv_fs_type_UV_FS_CHOWN => FileSystemRequestType::CHOWN,
            uv::uv_fs_type_UV_FS_FCHOWN => FileSystemRequestType::FCHOWN,
            uv::uv_fs_type_UV_FS_REALPATH => FileSystemRequestType::REALPATH,
            uv::uv_fs_type_UV_FS_COPYFILE => FileSystemRequestType::COPYFILE,
            uv::uv_fs_type_UV_FS_LCHOWN => FileSystemRequestType::LCHOWN,
            uv::uv_fs_type_UV_FS_OPENDIR => FileSystemRequestType::OPENDIR,
            uv::uv_fs_type_UV_FS_READDIR => FileSystemRequestType::READDIR,
            uv::uv_fs_type_UV_FS_CLOSEDIR => FileSystemRequestType::CLOSEDIR,
            uv::uv_fs_type_UV_FS_STATFS => FileSystemRequestType::STATFS,
            uv::uv_fs_type_UV_FS_MKSTEMP => FileSystemRequestType::MKSTEMP,
            uv::uv_fs_type_UV_FS_LUTIME => FileSystemRequestType::LUTIME,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<uv_fs_type> for FileSystemRequestType {
    fn into_inner(self) -> uv_fs_type {
        match self {
            FileSystemRequestType::UNKNOWN => uv::uv_fs_type_UV_FS_UNKNOWN,
            FileSystemRequestType::CUSTOM => uv::uv_fs_type_UV_FS_CUSTOM,
            FileSystemRequestType::OPEN => uv::uv_fs_type_UV_FS_OPEN,
            FileSystemRequestType::CLOSE => uv::uv_fs_type_UV_FS_CLOSE,
            FileSystemRequestType::READ => uv::uv_fs_type_UV_FS_READ,
            FileSystemRequestType::WRITE => uv::uv_fs_type_UV_FS_WRITE,
            FileSystemRequestType::SENDFILE => uv::uv_fs_type_UV_FS_SENDFILE,
            FileSystemRequestType::STAT => uv::uv_fs_type_UV_FS_STAT,
            FileSystemRequestType::LSTAT => uv::uv_fs_type_UV_FS_LSTAT,
            FileSystemRequestType::FSTAT => uv::uv_fs_type_UV_FS_FSTAT,
            FileSystemRequestType::FTRUNCATE => uv::uv_fs_type_UV_FS_FTRUNCATE,
            FileSystemRequestType::UTIME => uv::uv_fs_type_UV_FS_UTIME,
            FileSystemRequestType::FUTIME => uv::uv_fs_type_UV_FS_FUTIME,
            FileSystemRequestType::ACCESS => uv::uv_fs_type_UV_FS_ACCESS,
            FileSystemRequestType::CHMOD => uv::uv_fs_type_UV_FS_CHMOD,
            FileSystemRequestType::FCHMOD => uv::uv_fs_type_UV_FS_FCHMOD,
            FileSystemRequestType::FSYNC => uv::uv_fs_type_UV_FS_FSYNC,
            FileSystemRequestType::FDATASYNC => uv::uv_fs_type_UV_FS_FDATASYNC,
            FileSystemRequestType::UNLINK => uv::uv_fs_type_UV_FS_UNLINK,
            FileSystemRequestType::RMDIR => uv::uv_fs_type_UV_FS_RMDIR,
            FileSystemRequestType::MKDIR => uv::uv_fs_type_UV_FS_MKDIR,
            FileSystemRequestType::MKDTEMP => uv::uv_fs_type_UV_FS_MKDTEMP,
            FileSystemRequestType::RENAME => uv::uv_fs_type_UV_FS_RENAME,
            FileSystemRequestType::SCANDIR => uv::uv_fs_type_UV_FS_SCANDIR,
            FileSystemRequestType::LINK => uv::uv_fs_type_UV_FS_LINK,
            FileSystemRequestType::SYMLINK => uv::uv_fs_type_UV_FS_SYMLINK,
            FileSystemRequestType::READLINK => uv::uv_fs_type_UV_FS_READLINK,
            FileSystemRequestType::CHOWN => uv::uv_fs_type_UV_FS_CHOWN,
            FileSystemRequestType::FCHOWN => uv::uv_fs_type_UV_FS_FCHOWN,
            FileSystemRequestType::REALPATH => uv::uv_fs_type_UV_FS_REALPATH,
            FileSystemRequestType::COPYFILE => uv::uv_fs_type_UV_FS_COPYFILE,
            FileSystemRequestType::LCHOWN => uv::uv_fs_type_UV_FS_LCHOWN,
            FileSystemRequestType::OPENDIR => uv::uv_fs_type_UV_FS_OPENDIR,
            FileSystemRequestType::READDIR => uv::uv_fs_type_UV_FS_READDIR,
            FileSystemRequestType::CLOSEDIR => uv::uv_fs_type_UV_FS_CLOSEDIR,
            FileSystemRequestType::STATFS => uv::uv_fs_type_UV_FS_STATFS,
            FileSystemRequestType::MKSTEMP => uv::uv_fs_type_UV_FS_MKSTEMP,
            FileSystemRequestType::LUTIME => uv::uv_fs_type_UV_FS_LUTIME,
        }
    }
}

impl FromInner<u32> for OpenMode {
    fn from_inner(value: u32) -> Self {
        match value {
            uv::UV_FS_O_RDONLY => OpenMode::RDONLY,
            uv::UV_FS_O_WRONLY => OpenMode::WRONLY,
            uv::UV_FS_O_RDWR => OpenMode::RDWR,
            _ => unreachable!(),
        }
    }
}

impl IntoInner<u32> for OpenMode {
    fn into_inner(self) -> u32 {
        match self {
            OpenMode::RDONLY => uv::UV_FS_O_RDONLY,
            OpenMode::WRONLY => uv::UV_FS_O_WRONLY,
            OpenMode::RDWR => uv::UV_FS_O_RDWR,
        }
    }
}

impl FromInner<*mut uv_fs_t> for FileSystemRequest {
    fn from_inner(raw: *mut uv_fs_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_fs_t> for FileSystemRequest {
    fn into_inner(self) -> *mut uv_fs_t {
        self.raw
    }
}
