pub(crate) mod tty;

pub(crate) use tty::*;

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        AllocCallback, Buf, ConstBuf, Errno, IHandle, IRequest, WriteCallback, WriteContext,
        WriteRequest, uv_alloc_cb, uv_buf_t, uv_errno_t, uv_handle_t, uv_read_start, uv_read_stop,
        uv_stream_t, uv_write, uv_write_cb,
    },
};

impl<'a, B: Buf> super::IHandleContext<'a, B> for StreamContext<'a, B> {
    fn into_handle_context(self) -> super::HandleContext<'a, B> {
        super::HandleContext::from(self)
    }
}

impl<B: Buf> super::IHandle<B> for StreamHandle {
    fn into_handle(self) -> super::Handle<B> {
        super::Handle::from_raw(self.raw as *mut uv_handle_t)
    }
}

pub struct ReadCallback<'a, B: Buf>(pub Box<dyn FnMut(&'a StreamHandle, Result<isize, Errno>, B)>);

pub struct StreamContext<'a, B: Buf> {
    alloc_cb: Option<AllocCallback<'a, B>>,
    read_cb: Option<ReadCallback<'a, B>>,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamHandle {
    raw: *mut uv_stream_t,
}

pub trait IStreamHandle: Copy {
    fn into_stream(self) -> StreamHandle;

    // NOTE: `bufs` is expected to be deallocated by the caller
    fn write<B: Buf, WCB>(&mut self, req: WriteRequest, bufs: &[B], cb: WCB) -> Result<(), Errno>
    where
        WCB: Into<WriteCallback>,
    {
        match req.into_request().get_context() {
            Some(context) => {
                context.write_cb = Some(cb.into());
            }
            None => {
                let new_context = WriteContext {
                    write_cb: Some(cb.into()),
                };
                req.into_request().set_context(new_context);
            }
        };

        let bigbuf = bufs.into_inner();
        let result = unsafe {
            uv_write(
                req.into_inner(),
                self.into_stream().into_inner(),
                bigbuf.as_ptr(),
                bufs.len() as u32,
                Some(uv_write_cb),
            )
        };

        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    // NOTE: `buf` is expected to be deallocated in the read_cb
    fn read_start<'a, B: Buf + 'a, ACB, RCB>(
        &mut self,
        alloc_cb: ACB,
        read_cb: RCB,
    ) -> Result<(), Errno>
    where
        ACB: Into<AllocCallback<'a, B>>,
        RCB: Into<ReadCallback<'a, B>>,
    {
        match self
            .into_stream()
            .into_handle()
            .get_context::<StreamContext<B>>()
        {
            Some(ref mut context) => {
                context.alloc_cb = Some(alloc_cb.into());
                context.read_cb = Some(read_cb.into());
            }
            None => {
                let new_context = StreamContext {
                    alloc_cb: Some(alloc_cb.into()),
                    read_cb: Some(read_cb.into()),
                };
                self.into_stream().into_handle().set_context(new_context);
            }
        };

        let result = unsafe {
            uv_read_start(
                self.into_stream().into_inner(),
                Some(uv_alloc_cb::<B>),
                Some(uv_read_cb),
            )
        };

        if result < 0 {
            Err(Errno::from_inner(result))
        } else {
            Ok(())
        }
    }

    fn read_stop(&mut self) {
        unsafe { uv_read_stop(self.into_stream().into_inner()) };
    }
}

impl IStreamHandle for StreamHandle {
    fn into_stream(self) -> StreamHandle {
        self
    }
}

pub(crate) fn init_stream(raw: *mut uv_stream_t) {
    super::init_handle(raw as *mut uv_handle_t);
}

impl StreamHandle {
    pub(crate) fn from_raw(raw: *mut uv_stream_t) -> Self {
        Self { raw }
    }
}

pub(crate) unsafe extern "C" fn uv_read_cb(
    stream: *mut uv_stream_t,
    nread: isize,
    buf: *const uv_buf_t,
) {
    let stream = StreamHandle::from_inner(stream);
    if let Some(context) = stream
        .into_handle()
        .get_context::<StreamContext<ConstBuf>>()
    {
        let status = if nread < 0 {
            Err(Errno::from_inner(nread as uv_errno_t))
        } else {
            Ok(nread)
        };

        if let Some(ref mut read_cb) = context.read_cb {
            read_cb.0(&stream, status, ConstBuf::from_raw(buf));
        }
        // drop(Box::from_raw(buf as *mut uv_buf_t)); // FIXME: why should buf not be deallocated here?
    }
}

impl<'a, Fn, B: Buf> From<Fn> for ReadCallback<'a, B>
where
    Fn: FnMut(&StreamHandle, Result<isize, Errno>, B) + 'static,
{
    fn from(value: Fn) -> Self {
        Self(Box::new(value))
    }
}

impl<'a, B: Buf> From<()> for ReadCallback<'a, B> {
    fn from(_: ()) -> Self {
        Self(Box::new(|_, _, _| {}))
    }
}

impl<'a, B: Buf> From<StreamContext<'a, B>> for super::HandleContext<'a, B> {
    fn from(value: StreamContext<'a, B>) -> Self {
        Self {
            alloc_cb: value.alloc_cb,
        }
    }
}

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
