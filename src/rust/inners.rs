use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub(crate) struct NullPtrError();

impl Display for NullPtrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "null ptr error")
    }
}

impl Error for NullPtrError {}

pub(crate) trait FromInner<T>: Sized {
    fn from_inner(value: T) -> Self;
}

pub(crate) trait IntoInner<T>: Sized {
    fn into_inner(self) -> T;
}
