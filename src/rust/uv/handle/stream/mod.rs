pub(crate) mod write;
pub(crate) use write::*;

pub(crate) mod tty;
pub(crate) use tty::*;

use crate::uv::{Buf, Errno, uv_buf_t, uv_stream_t, uv_write};

pub struct Stream {
    raw: *mut uv_stream_t,
}

pub trait StreamHandle {
    fn into_stream(&self) -> Stream;

    fn write(&mut self, req: &WriteRequest, bufs: &[Buf]) -> Result<(), Errno> {
        let bs: Vec<uv_buf_t> = unsafe {
            bufs.iter()
                .map(|b| *(Into::<*mut uv_buf_t>::into(b)))
                .collect()
        };

        let result = unsafe {
            uv_write(
                req.into(),
                self.into_stream().into(),
                bs.as_ptr(),
                bufs.len() as u32,
                Some(uv_write_cb),
            )
        };

        if result < 0 {
            Err(Errno::from(result))
        } else {
            Ok(())
        }
    }
}

impl From<*mut uv_stream_t> for Stream {
    fn from(raw: *mut uv_stream_t) -> Self {
        Self { raw }
    }
}

impl Into<*mut uv_stream_t> for Stream {
    fn into(self) -> *mut uv_stream_t {
        self.raw
    }
}

impl Stream {
    pub(crate) fn from_raw(raw: *mut uv_stream_t) -> Self {
        Self { raw }
    }
}
