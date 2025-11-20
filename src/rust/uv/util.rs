use crate::{
    inners::FromInner,
    uv::{self, uv_guess_handle, uv_handle_type},
};

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq)]
pub enum HandleType {
    UNKNOWN_HANDLE,
    ASYNC,
    CHECK,
    FS_EVENT,
    FS_POLL,
    HANDLE,
    IDLE,
    NAMED_PIPE,
    POLL,
    PREPARE,
    PROCESS,
    STREAM,
    TCP,
    TIMER,
    TTY,
    UDP,
    SIGNAL,
    FILE,
    HANDLE_TYPE_MAX,
}

impl FromInner<uv_handle_type> for HandleType {
    fn from_inner(value: uv_handle_type) -> Self {
        match value {
            uv::uv_handle_type_UV_UNKNOWN_HANDLE => HandleType::UNKNOWN_HANDLE,
            uv::uv_handle_type_UV_ASYNC => HandleType::ASYNC,
            uv::uv_handle_type_UV_CHECK => HandleType::CHECK,
            uv::uv_handle_type_UV_FS_EVENT => HandleType::FS_EVENT,
            uv::uv_handle_type_UV_FS_POLL => HandleType::FS_POLL,
            uv::uv_handle_type_UV_HANDLE => HandleType::HANDLE,
            uv::uv_handle_type_UV_IDLE => HandleType::IDLE,
            uv::uv_handle_type_UV_NAMED_PIPE => HandleType::NAMED_PIPE,
            uv::uv_handle_type_UV_POLL => HandleType::POLL,
            uv::uv_handle_type_UV_PREPARE => HandleType::PREPARE,
            uv::uv_handle_type_UV_PROCESS => HandleType::PROCESS,
            uv::uv_handle_type_UV_STREAM => HandleType::STREAM,
            uv::uv_handle_type_UV_TCP => HandleType::TCP,
            uv::uv_handle_type_UV_TIMER => HandleType::TIMER,
            uv::uv_handle_type_UV_TTY => HandleType::TTY,
            uv::uv_handle_type_UV_UDP => HandleType::UDP,
            uv::uv_handle_type_UV_SIGNAL => HandleType::SIGNAL,
            uv::uv_handle_type_UV_FILE => HandleType::FILE,
            uv::uv_handle_type_UV_HANDLE_TYPE_MAX => HandleType::HANDLE_TYPE_MAX,
            _ => unreachable!(),
        }
    }
}

pub fn guess_handle(fd: i32) -> HandleType {
    HandleType::from_inner(unsafe { uv_guess_handle(fd) })
}
