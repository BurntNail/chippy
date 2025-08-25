use crate::events::{EventReadError, TEXT_MESSAGE};
use crate::integer::{Integer, IntegerDeserialiser, SignedState};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

#[derive(Clone, Debug)]
pub enum EventToClient {
    TxtSent {
        name: String,
        content: String
    }
}

impl Serable for EventToClient {
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        match self {
            EventToClient::TxtSent { name, content } => {
                into.push(TEXT_MESSAGE); //message kind

                Integer::from(name.len()).ser_into(into);
                Integer::from(content.len()).ser_into(into); //can skip signed states because always unsigned

                into.extend_from_slice(name.as_bytes());
                into.extend_from_slice(content.as_bytes());
            }
        }

        ()
    }
}

impl Deserable for EventToClient {
    type Deserer = ClientEventDeserer;
}

pub enum ClientEventDeserer {
    Start(u8),
    GotStart(u8),
    DeseringTxtMsg(TxtDeserer),
}

impl DeserMachine for ClientEventDeserer {
    type StartingInput = ();
    type Output = EventToClient;
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
                TEXT_MESSAGE => Ok(FsmResult::Continue(Self::DeseringTxtMsg(TxtDeserer::new()))),
                n => Err(EventReadError::InvalidKind(n)),
            }
            Self::DeseringTxtMsg(deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::DeseringTxtMsg(deser))),
                FsmResult::Done(msg) => Ok(FsmResult::Done(msg))
            }
        }
    }
}

pub enum TxtDeserer {
    DeseringNameLen(IntegerDeserialiser),
    DeseringContentLen(usize, IntegerDeserialiser),
    ReadingName {
        name_bytes_left: usize,
        content_bytes: usize,
        name_so_far: Vec<u8>,
    },
    ReadingContent {
        name: String,
        content_bytes_left: usize,
        content_so_far: Vec<u8>
    }
}

impl DeserMachine for TxtDeserer {
    type StartingInput = ();
    type Output = EventToClient;
    type Error = EventReadError;

    fn new() -> Self {
        Self::DeseringNameLen(Integer::deser_with_input(SignedState::Unsigned))
    }

    fn new_with_starting_input((): Self::StartingInput) -> Self {
        Self::new()
    }

    fn wants_read(&mut self) -> DesiredInput {
        match self {
            Self::DeseringNameLen(deser) => {
                deser.wants_read()
            }
            Self::DeseringContentLen(_, deser) => {
                deser.wants_read()
            }
            Self::ReadingName { name_bytes_left, content_bytes: _, name_so_far } => {
                if *name_bytes_left == 0 {
                    DesiredInput::ProcessMe
                } else {
                    let start_index = name_so_far.len() - *name_bytes_left;
                    DesiredInput::Bytes(&mut name_so_far[start_index..])
                }
            }
            Self::ReadingContent { name: _, content_bytes_left, content_so_far } => {
                if *content_bytes_left == 0 {
                    DesiredInput::ProcessMe
                } else {
                    let start_index = content_so_far.len() - *content_bytes_left;
                    DesiredInput::Bytes(&mut content_so_far[start_index..])
                }
            }
        }
    }

    fn give_starting_input(&mut self, (): Self::StartingInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) { //TODO: consistency between this and integer for where logic is done? needs more experimentation with this style
        match self {
            Self::DeseringNameLen(deser) => deser.finish_bytes_for_writing(n),
            Self::DeseringContentLen(_, deser) => deser.finish_bytes_for_writing(n),
            Self::ReadingName { name_bytes_left, .. } => {
                *name_bytes_left -= n;
            }
            Self::ReadingContent { content_bytes_left, .. } => {
                *content_bytes_left -= n;
            }
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::DeseringNameLen(deser) => match deser.process()? {
                FsmResult::Continue(deser) => {
                    Ok(FsmResult::Continue(Self::DeseringNameLen(deser)))
                }
                FsmResult::Done(int) => {
                    let len = int.try_into()?;
                    Ok(FsmResult::Continue(Self::DeseringContentLen(len, Integer::deser())))
                }
            }
            Self::DeseringContentLen(name_bytes_left, deser) => match deser.process()? {
                FsmResult::Continue(deser) => {
                    Ok(FsmResult::Continue(Self::DeseringContentLen(name_bytes_left, deser)))
                }
                FsmResult::Done(int) => {
                    let content_bytes = int.try_into()?;
                    Ok(FsmResult::Continue(Self::ReadingName {
                        name_bytes_left,
                        content_bytes,
                        name_so_far: vec![0; name_bytes_left],
                    }))
                },
            }
            Self::ReadingName { name_bytes_left, content_bytes, name_so_far } => {
                if name_bytes_left == 0 {
                    let name = String::from_utf8(name_so_far)?;
                    Ok(FsmResult::Continue(Self::ReadingContent {
                        name,
                        content_bytes_left: content_bytes,
                        content_so_far: vec![0; content_bytes],
                    }))
                } else {
                    Ok(FsmResult::Continue(Self::ReadingName {
                        name_bytes_left, content_bytes, name_so_far
                    }))
                }
            }
            Self::ReadingContent { name, content_bytes_left, content_so_far } => {
                if content_bytes_left == 0 {
                    let content = String::from_utf8(content_so_far)?;
                    Ok(FsmResult::Done(EventToClient::TxtSent {
                        name,
                        content,
                    })) //yipee!!!
                } else {
                    Ok(FsmResult::Continue(Self::ReadingContent {
                        name,
                        content_bytes_left,
                        content_so_far,
                    }))
                }
            }
        }
    }
}
