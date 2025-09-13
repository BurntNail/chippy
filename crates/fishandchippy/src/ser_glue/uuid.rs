use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use std::convert::Infallible;
use uuid::Uuid;

impl Serable for Uuid {
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        into.extend_from_slice(self.as_bytes());
    }
}

#[derive(Debug)]
pub struct UuidDeserialiser {
    content: [u8; 16],
    bytes_left: usize,
}

impl Deserable for Uuid {
    type Deserer = UuidDeserialiser;
}

impl DeserMachine for UuidDeserialiser {
    type ExtraInput = ();
    type Output = Uuid;
    type Error = Infallible;

    fn new() -> Self {
        Self {
            content: [0; 16],
            bytes_left: 16,
        }
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        if self.bytes_left == 0 {
            DesiredInput::ProcessMe
        } else {
            let start_index = self.content.len() - self.bytes_left;
            DesiredInput::Bytes(&mut self.content[start_index..])
        }
    }

    fn give_starting_input(&mut self, (): Self::ExtraInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) {
        self.bytes_left -= n;
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        if self.bytes_left == 0 {
            let uuid = Uuid::from_slice(&self.content).unwrap(); //unwrap: ok because always 16 len
            Ok(FsmResult::Done(uuid))
        } else {
            Ok(FsmResult::Continue(self))
        }
    }
}
