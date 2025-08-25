use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;
use crate::integer::{Integer, IntegerDeserialiser, IntegerReadError};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult};

pub enum StringDeserer {
    DeseringLen(IntegerDeserialiser),
    ReadingContent {
        bytes_left: usize,
        content_so_far: Vec<u8>
    }
}

#[derive(Debug)]
pub enum StringReadError {
    Integer(IntegerReadError),
    String(FromUtf8Error)
}

impl From<IntegerReadError> for StringReadError {
    fn from(value: IntegerReadError) -> Self {
        Self::Integer(value)
    }
}
impl From<FromUtf8Error> for StringReadError {
    fn from(value: FromUtf8Error) -> Self {
        Self::String(value)
    }
}

impl Display for StringReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StringReadError::Integer(int) => write!(f, "Error reading length: {int}"),
            StringReadError::String(string) => write!(f, "Error reading content as UTF-8: {string}"),
        }
    }
}

impl std::error::Error for StringReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
         match self {
             StringReadError::Integer(int) => Some(int),
             StringReadError::String(string) => Some(string),
         }
    }
}

impl DeserMachine for StringDeserer {
    type StartingInput = ();
    type Output = String;
    type Error = StringReadError;

    fn new() -> Self {
        Self::DeseringLen(Integer::deser())
    }

    fn new_with_starting_input((): Self::StartingInput) -> Self {
        Self::new()
    }

    fn wants_read(&mut self) -> DesiredInput {
        match self {
            Self::DeseringLen(deser) => deser.wants_read(),
            Self::ReadingContent {
                bytes_left, content_so_far
            } => {
                if *bytes_left == 0 {
                    DesiredInput::ProcessMe
                } else {
                    let start_index = content_so_far.len() - *bytes_left;
                    DesiredInput::Bytes(&mut content_so_far[start_index..])
                }
            }
        }
    }

    fn give_starting_input(&mut self, (): Self::StartingInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match self {
            Self::DeseringLen(deser) => deser.finish_bytes_for_writing(n),
            Self::ReadingContent { bytes_left, .. } => {
                *bytes_left -= n;
            }
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::DeseringLen(deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::DeseringLen(deser))),
                FsmResult::Done(int) => {
                    let bytes_left = int.try_into()?;
                    Ok(FsmResult::Continue(Self::ReadingContent {
                        bytes_left,
                        content_so_far: vec![0; bytes_left],
                    }))
                }
            }
            Self::ReadingContent { bytes_left, content_so_far } => {
                if bytes_left == 0 {
                    let content = String::from_utf8(content_so_far)?;
                    Ok(FsmResult::Done(content))
                } else {
                    Ok(FsmResult::Continue(Self::ReadingContent {
                        bytes_left,
                        content_so_far,
                    }))
                }
            }
        }
    }
}