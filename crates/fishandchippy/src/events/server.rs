use crate::events::{EventReadError, TEXT_MESSAGE};
use crate::integer::{Integer};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use crate::ser_glue::string::StringDeserer;

#[derive(Clone, Debug)]
pub enum EventToServer {
    SendMessage(String)
}

impl Serable for EventToServer {
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        match self {
            EventToServer::SendMessage(msg) => {
                into.push(TEXT_MESSAGE);

                Integer::from(msg.len()).ser_into(into); //can ignore signed state as is always unsigned
                into.extend_from_slice(msg.as_bytes());
            }
        }
    }
}

impl Deserable for EventToServer {
    type Deserer = ServerEventDeserer;
}

pub enum ServerEventDeserer {
    Start(u8),
    GotStart(u8),
    DeseringTxtMsg(StringDeserer)
}

impl DeserMachine for ServerEventDeserer {
    type StartingInput = ();
    type Output = EventToServer;
    type Error = EventReadError;

    fn new() -> Self {
        Self::Start(0)
    }

    fn new_with_starting_input((): Self::StartingInput) -> Self {
        Self::new()
    }

    fn wants_read(&mut self) -> DesiredInput {
        match self {
            Self::Start(space) => DesiredInput::Byte(space),
            Self::GotStart(_start) => DesiredInput::ProcessMe,
            Self::DeseringTxtMsg(txt_deser) => txt_deser.wants_read(),
        }
    }

    fn give_starting_input(&mut self, (): Self::StartingInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) {
        *self = match std::mem::replace(self, Self::new()) {
            Self::Start(start) => {
                if n == 1 {
                    Self::GotStart(start)
                } else {
                    Self::Start(start)
                }
            }
            Self::DeseringTxtMsg(mut deser) => {
                deser.finish_bytes_for_writing(n);
                Self::DeseringTxtMsg(deser)
            },
            waiting => waiting,
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::Start(n) => Ok(FsmResult::Continue(Self::Start(n))),
            Self::GotStart(n) => match n {
                TEXT_MESSAGE => Ok(FsmResult::Continue(Self::DeseringTxtMsg(StringDeserer::new()))),
                n => Err(EventReadError::InvalidKind(n)),
            }
            Self::DeseringTxtMsg(deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::DeseringTxtMsg(deser))),
                FsmResult::Done(msg) => Ok(FsmResult::Done(EventToServer::SendMessage(msg)))
            }
        }
    }
}

