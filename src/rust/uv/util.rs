use crate::{
    inners::FromInner,
    uv::{HandleType, uv_guess_handle},
};

pub fn guess_handle(fd: i32) -> HandleType {
    HandleType::from_inner(unsafe { uv_guess_handle(fd) })
}
