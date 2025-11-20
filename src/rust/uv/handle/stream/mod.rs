pub(crate) mod tty;
pub(crate) use tty::*;

use crate::{
    inners::{FromInner, IntoInner},
    uv::{
        Errno, IRequest, MutBuf, WriteCallback, WriteContext, WriteRequest, uv_stream_t, uv_write,
        uv_write_cb,
    },
};

pub struct StreamHandle {
    raw: *mut uv_stream_t,
}

pub trait IStreamHandle {
    fn into_stream(&self) -> StreamHandle;

    fn write<CB>(&mut self, req: &WriteRequest, bufs: &[MutBuf], cb: CB) -> Result<(), Errno>
    where
        CB: Into<WriteCallback>,
    {
        let context = req.into_request().get_context();
        let new_context = match context {
            Some(mut context) => {
                context.cb = Some(cb.into());
                context
            }
            None => Box::new(WriteContext {
                cb: Some(cb.into()),
            }),
        };
        req.into_request().set_context(new_context);

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

impl StreamHandle {
    pub(crate) fn from_raw(raw: *mut uv_stream_t) -> Self {
        Self { raw }
    }
}
