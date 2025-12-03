// mod

pub(crate) mod tty;
pub(crate) use tty::*;

use std::{
    any::{Any, TypeId},
    os::raw::{c_int, c_void},
    ptr::null_mut,
};

use crate::{
    inners::{FromInner, IntoInner},
    result,
    uv::{
        AllocCallback, Buf, CloseCallback, Errno, Handle, IHandle, IRequest, ShutdownCallback,
        ShutdownContext, ShutdownRequest, WriteCallback, WriteContext, WriteRequest, uv_accept,
        uv_alloc_cb, uv_buf_t, uv_errno_t, uv_handle_t, uv_is_readable, uv_is_writable, uv_listen,
        uv_read_start, uv_read_stop, uv_shutdown, uv_shutdown_cb, uv_stream_t, uv_tty_t, uv_write,
        uv_write_cb,
    },
};

// super

impl<'a> super::IHandleContext<'a> for StreamContext<'a> {
    fn into_handle_context(self) -> super::HandleContext<'a> {
        super::HandleContext::from(self)
    }
}

impl super::IHandle for StreamHandle {
    fn into_handle(self) -> super::Handle {
        super::Handle::from_inner(self.raw as *mut uv_handle_t)
    }

    fn drop_handle(self) {
        self.drop_stream();
    }
}

// type

pub struct ConnectionCallback<'a>(pub Box<dyn FnMut(&'a StreamHandle, Result<(), Errno>) + 'a>);
pub struct ReadCallback<'a>(pub Box<dyn FnMut(&'a StreamHandle, Result<isize, Errno>, Buf) + 'a>);

#[repr(C)]
pub struct StreamContext<'a> {
    alloc_cb: Option<AllocCallback<'a>>,
    close_cb: Option<CloseCallback<'a>>,
    data: *mut c_void,
    connection_cb: Option<ConnectionCallback<'a>>,
    read_cb: Option<ReadCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamHandle {
    raw: *mut uv_stream_t,
}

pub trait IStreamHandle: Copy {
    fn into_stream(self) -> StreamHandle;

    fn drop_stream(self);

    fn shutdown<'a, SCB>(&mut self, req: ShutdownRequest, shutdown_cb: SCB) -> Result<(), Errno>
    where
        SCB: Into<ShutdownCallback<'a>>,
    {
        self.into_stream().shutdown(req, shutdown_cb)
    }

    fn listen<'a, CCB>(&mut self, backlog: i32, connection_cb: CCB) -> Result<(), Errno>
    where
        CCB: Into<ConnectionCallback<'a>>,
    {
        self.into_stream().listen(backlog, connection_cb)
    }

    fn accept(&mut self, client: &mut Self) -> Result<(), Errno> {
        self.into_stream().accept(&mut client.into_stream())
    }

    fn read_start<'a, ACB, RCB>(&mut self, alloc_cb: ACB, read_cb: RCB) -> Result<(), Errno>
    where
        ACB: Into<AllocCallback<'a>>,
        RCB: Into<ReadCallback<'a>>,
    {
        self.into_stream().read_start(alloc_cb, read_cb)
    }

    fn read_stop(&mut self) {
        self.into_stream().read_stop()
    }

    fn write<'a, WCB>(
        &mut self,
        req: WriteRequest,
        bufs: &[Buf],
        write_cb: WCB,
    ) -> Result<(), Errno>
    where
        WCB: Into<WriteCallback<'a>>,
    {
        self.into_stream().write(req, bufs, write_cb)
    }

    fn readable(&self) -> bool {
        self.into_stream().readable()
    }

    fn writable(&self) -> bool {
        self.into_stream().writable()
    }
}

// fn

pub(crate) fn init_stream(raw: *mut uv_stream_t) {
    super::init_handle(raw as *mut uv_handle_t);
}

pub(crate) unsafe extern "C" fn uv_connection_cb(stream: *mut uv_stream_t, status: c_int) {
    let stream = StreamHandle::from_inner(stream);
    if let Some(context) = stream.into_handle().get_context::<StreamContext>() {
        let status = if status < 0 {
            Err(Errno::from_inner(status))
        } else {
            Ok(())
        };

        if let Some(ref mut connection_cb) = context.connection_cb {
            connection_cb.0(&stream, status);
        }
    }
}

pub(crate) unsafe extern "C" fn uv_read_cb(
    stream: *mut uv_stream_t,
    nread: isize,
    buf: *const uv_buf_t,
) {
    let stream = StreamHandle::from_inner(stream);
    if let Some(context) = stream.into_handle().get_context::<StreamContext>() {
        let status = if nread < 0 {
            Err(Errno::from_inner(nread as uv_errno_t))
        } else {
            Ok(nread)
        };

        if let Some(ref mut read_cb) = context.read_cb {
            read_cb.0(&stream, status, Buf::from_inner(buf as *mut uv_buf_t));
        }
        drop(Box::from_raw(buf as *mut uv_buf_t))
    }
}

// impl

impl StreamHandle {
    pub fn shutdown<'a, SCB>(&mut self, req: ShutdownRequest, shutdown_cb: SCB) -> Result<(), Errno>
    where
        SCB: Into<ShutdownCallback<'a>>,
    {
        let mut request = req.into_request();
        match unsafe { request.get_context::<ShutdownContext>() } {
            Some(context) => {
                context.shutdown_cb = Some(shutdown_cb.into());
            }
            None => request.set_context(ShutdownContext {
                data: null_mut(),
                shutdown_cb: Some(shutdown_cb.into()),
            }),
        };

        result!(unsafe { uv_shutdown(req.into_inner(), self.raw, Some(uv_shutdown_cb)) })
    }

    pub fn listen<'a, CCB>(&mut self, backlog: i32, connection_cb: CCB) -> Result<(), Errno>
    where
        CCB: Into<ConnectionCallback<'a>>,
    {
        let mut handle = self.into_handle();
        match unsafe { handle.get_context::<StreamContext>() } {
            Some(context) => {
                context.connection_cb = Some(connection_cb.into());
            }
            None => {
                handle.set_context(StreamContext {
                    alloc_cb: None,
                    close_cb: None,
                    data: null_mut(),
                    connection_cb: Some(connection_cb.into()),
                    read_cb: None,
                });
            }
        };

        result!(unsafe { uv_listen(self.raw, backlog, Some(uv_connection_cb)) })
    }

    pub fn accept(&mut self, client: &mut Self) -> Result<(), Errno> {
        result!(unsafe { uv_accept(self.raw, client.into_inner()) })
    }

    pub fn read_start<'a, ACB, RCB>(&mut self, alloc_cb: ACB, read_cb: RCB) -> Result<(), Errno>
    where
        ACB: Into<AllocCallback<'a>>,
        RCB: Into<ReadCallback<'a>>,
    {
        let mut handle = self.into_handle();
        match unsafe { handle.get_context::<StreamContext>() } {
            Some(context) => {
                context.alloc_cb = Some(alloc_cb.into());
                context.read_cb = Some(read_cb.into());
            }
            None => {
                handle.set_context(StreamContext {
                    alloc_cb: Some(alloc_cb.into()),
                    close_cb: None,
                    data: null_mut(),
                    connection_cb: None,
                    read_cb: Some(read_cb.into()),
                });
            }
        };

        result!(unsafe { uv_read_start(self.raw, Some(uv_alloc_cb), Some(uv_read_cb)) })
    }

    pub fn read_stop(&mut self) {
        unsafe { uv_read_stop(self.raw) };
    }

    pub fn write<'a, WCB>(
        &mut self,
        req: WriteRequest,
        bufs: &[Buf],
        write_cb: WCB,
    ) -> Result<(), Errno>
    where
        WCB: Into<WriteCallback<'a>>,
    {
        let mut request = req.into_request();
        match unsafe { request.get_context::<WriteContext>() } {
            Some(context) => {
                context.write_cb = Some(write_cb.into());
            }
            None => request.set_context(WriteContext {
                data: null_mut(),
                write_cb: Some(write_cb.into()),
            }),
        };

        let (bufs, nbufs) = bufs.into_inner();
        result!(unsafe {
            uv_write(
                req.into_inner(),
                self.raw,
                bufs,
                nbufs as u32,
                Some(uv_write_cb),
            )
        })
    }

    pub fn readable(&self) -> bool {
        unsafe { uv_is_readable(self.raw) != 0 }
    }

    pub fn writable(&self) -> bool {
        unsafe { uv_is_writable(self.raw) != 0 }
    }

    pub fn get_data<D: 'static>(&self) -> Option<&mut D> {
        let handle = self.into_handle();
        if let Some(context) = unsafe { handle.get_context::<StreamContext>() } {
            Some(unsafe {
                (*(context.data as *mut dyn Any))
                    .downcast_mut::<D>()
                    .expect(&format!(
                        "StreamHandle::get_data: unexpected type, expected: [{:?}] but was: [{:?}]",
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
        match unsafe { handle.get_context::<StreamContext>() } {
            Some(context) => context.data = data,
            None => {
                handle.set_context(StreamContext {
                    alloc_cb: None,
                    close_cb: None,
                    data,
                    connection_cb: None,
                    read_cb: None,
                });
            }
        }
    }
}

impl IStreamHandle for StreamHandle {
    fn into_stream(self) -> StreamHandle {
        self
    }

    fn drop_stream(self) {
        match self.get_type() {
            crate::uv::HandleType::TTY => {
                TTYStream::from_inner(self.raw as *mut uv_tty_t).drop_stream()
            }
            _ => panic!(
                "StreamHandle::drop_stream: unexpected type [{}]",
                self.get_type().name()
            ),
        }
    }
}

// trait

impl<'a> From<StreamContext<'a>> for super::HandleContext<'a> {
    fn from(value: StreamContext<'a>) -> Self {
        Self {
            alloc_cb: value.alloc_cb,
            close_cb: value.close_cb,
            data: value.data,
        }
    }
}

impl<'a, Fn> From<Fn> for ConnectionCallback<'a>
where
    Fn: FnMut(&StreamHandle, Result<(), Errno>) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for ConnectionCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _| {}))
    }
}

impl<'a, Fn> From<Fn> for ReadCallback<'a>
where
    Fn: FnMut(&StreamHandle, Result<isize, Errno>, Buf) + 'a,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a> From<()> for ReadCallback<'a> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _, _| {}))
    }
}

// from_inner/into_inner

impl FromInner<*mut uv_stream_t> for StreamHandle {
    fn from_inner(raw: *mut uv_stream_t) -> Self {
        Self { raw }
    }
}

impl IntoInner<*mut uv_stream_t> for StreamHandle {
    fn into_inner(self) -> *mut uv_stream_t {
        self.raw
    }
}
