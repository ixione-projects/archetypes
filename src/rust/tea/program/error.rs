use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum ProgramError {
    InitError(String),
    InternalError(Box<dyn Error>),
}

impl Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitError(msg) => write!(f, "Program::init: {}", msg),
            Self::InternalError(errno) => errno.fmt(f),
        }
    }
}

impl<T> From<T> for ProgramError
where
    T: Error + 'static,
{
    fn from(value: T) -> Self {
        Self::InternalError(Box::new(value))
    }
}
