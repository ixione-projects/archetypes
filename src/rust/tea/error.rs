use std::{error::Error, fmt::Display};

use crate::uv::{Errno, HandleType};

#[derive(Debug, PartialEq, Eq)]
pub enum TUIError {
    InvalidHandleType(HandleType, HandleType),
    InternalUVError(Errno),
}

impl Display for TUIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHandleType(expected, actual) => {
                write!(f, "expected {:?} but found {:?}", expected, actual)
            }
            Self::InternalUVError(errno) => {
                write!(f, "{}", errno)
            }
        }
    }
}

impl Error for TUIError {}

impl From<Errno> for TUIError {
    fn from(value: Errno) -> Self {
        Self::InternalUVError(value)
    }
}
