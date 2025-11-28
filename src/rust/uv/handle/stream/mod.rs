pub(crate) mod tty;

pub(crate) use tty::*;

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        AllocCallback, Buf, CloseCallback, ConstBuf, Errno, IHandle, IRequest, WriteCallback,
        WriteContext, WriteRequest, uv_alloc_cb, uv_buf_t, uv_errno_t, uv_handle_t, uv_read_start,
        uv_read_stop, uv_stream_t, uv_tty_t, uv_write, uv_write_cb,
    },
};

impl<'a> super::IHandleContext<'a> for StreamContext<'a> {
    fn into_handle_context(self) -> super::HandleContext<'a> {
        super::HandleContext::from(self)
    }
}

impl super::IHandle for StreamHandle {
    fn into_handle(self) -> super::Handle {
        super::Handle::from_inner(self.raw as *mut uv_handle_t)
    }

    fn free_handle(self) {
        self.free_stream();
    }
}

pub struct ReadCallback<'a>(
    pub Box<dyn FnMut(&'a StreamHandle, Result<isize, Errno>, ConstBuf) + 'a>,
);

pub struct StreamContext<'a> {
    alloc_cb: Option<AllocCallback<'a>>,
    close_cb: Option<CloseCallback<'a>>,
    read_cb: Option<ReadCallback<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamHandle {
    raw: *mut uv_stream_t,
}

pub trait IStreamHandle: Copy {
    fn into_stream(self) -> StreamHandle;
    fn free_stream(self);

    // NOTE: `bufs` is expected to be deallocated by the caller
    fn write<'a, B: Buf, WCB>(
        &mut self,
        mut req: WriteRequest,
        bufs: &[B],
        write_cb: WCB,
    ) -> Result<(), Errno>
    where
        WCB: Into<WriteCallback<'a>>,
    {
        match req.get_context() {
            Some(context) => {
                context.write_cb = Some(write_cb.into());
            }
            None => {
                let new_context = WriteContext {
                    write_cb: Some(write_cb.into()),
                };
                req.set_context(new_context);
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

    fn read_start<'a, ACB, RCB>(&mut self, alloc_cb: ACB, read_cb: RCB) -> Result<(), Errno>
    where
        ACB: Into<AllocCallback<'a>>,
        RCB: Into<ReadCallback<'a>>,
    {
        match self.into_stream().get_context::<StreamContext>() {
            Some(ref mut context) => {
                context.alloc_cb = Some(alloc_cb.into());
                context.read_cb = Some(read_cb.into());
            }
            None => {
                let new_context = StreamContext {
                    alloc_cb: Some(alloc_cb.into()),
                    close_cb: None,
                    read_cb: Some(read_cb.into()),
                };
                self.into_stream().set_context(new_context);
            }
        };

        let result = unsafe {
            uv_read_start(
                self.into_stream().into_inner(),
                Some(uv_alloc_cb),
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

    fn free_stream(self) {
        match self.get_type() {
            crate::uv::HandleType::TTY => {
                TTYStream::from_inner(self.raw as *mut uv_tty_t).free_stream()
            }
            _ => panic!("unexpected handle type [{}]", self.get_type().name()),
        }
    }
}

pub(crate) fn init_stream(raw: *mut uv_stream_t) {
    super::init_handle(raw as *mut uv_handle_t);
}

pub(crate) unsafe extern "C" fn uv_read_cb(
    stream: *mut uv_stream_t,
    nread: isize,
    buf: *const uv_buf_t,
) {
    let stream = StreamHandle::from_inner(stream);
    if let Some(context) = stream.get_context::<StreamContext>() {
        let status = if nread < 0 {
            Err(Errno::from_inner(nread as uv_errno_t))
        } else {
            Ok(nread)
        };

        if let Some(ref mut read_cb) = context.read_cb {
            read_cb.0(&stream, status, ConstBuf::from_raw(buf));
        }
        // drop(Box::from_raw(buf as *mut uv_buf_t)); // FIXME: in read callback, buf should not be passing ownership of buf pointer to rust
    }
}

impl<'a, Fn> From<Fn> for ReadCallback<'a>
where
    Fn: FnMut(&StreamHandle, Result<isize, Errno>, ConstBuf) + 'a,
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

impl<'a> From<StreamContext<'a>> for super::HandleContext<'a> {
    fn from(value: StreamContext<'a>) -> Self {
        Self {
            alloc_cb: value.alloc_cb,
            close_cb: value.close_cb,
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
