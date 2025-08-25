use crate::integer::IntegerReadError;
use crate::ser_glue::string::StringReadError;
use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;

pub mod client;
pub mod server;

const TEXT_MESSAGE: u8 = 1;
const QUIT: u8 = 2;

#[derive(Debug)]
pub enum EventReadError {
    InvalidString(FromUtf8Error),
    Integer(IntegerReadError),
    InvalidKind(u8),
    StringRead(StringReadError),
}

impl From<FromUtf8Error> for EventReadError {
    fn from(value: FromUtf8Error) -> Self {
        Self::InvalidString(value)
    }
}
impl From<IntegerReadError> for EventReadError {
    fn from(value: IntegerReadError) -> Self {
        Self::Integer(value)
    }
}
impl From<StringReadError> for EventReadError {
    fn from(value: StringReadError) -> Self {
        Self::StringRead(value)
    }
}

impl Display for EventReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidString(str) => write!(f, "Error reading UTF-8: {str}"),
            Self::Integer(int) => write!(f, "Error parsing integer value: {int}"),
            Self::InvalidKind(kind) => write!(f, "Invalid event type provided: {kind}"),
            Self::StringRead(str) => write!(f, "Error reading basic string: {str}"),
        }
    }
}

impl std::error::Error for EventReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidString(str) => Some(str),
            Self::Integer(int) => Some(int),
            Self::StringRead(str) => Some(str),
            Self::InvalidKind(_) => None,
        }
    }
}
