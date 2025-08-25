use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;
use crate::integer::IntegerReadError;
use crate::ser_glue::string::StringReadError;

pub mod server;
pub mod client;

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
            EventReadError::InvalidString(str) => write!(f, "Error reading UTF-8: {str}"),
            EventReadError::Integer(int) => write!(f, "Error parsing integer value: {int}"),
            EventReadError::InvalidKind(kind) => write!(f, "Invalid event type provided: {kind}"),
            EventReadError::StringRead(str) => write!(f, "Error reading basic string: {str}"),
        }
    }
}

impl std::error::Error for EventReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EventReadError::InvalidString(str) => Some(str),
            EventReadError::Integer(int) => Some(int),
            EventReadError::StringRead(str) => Some(str),
            EventReadError::InvalidKind(_) => None,
        }
    }
}