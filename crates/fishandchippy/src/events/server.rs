use uuid::Uuid;
use crate::events::{EventReadError, INTRODUCTION, TEXT_MESSAGE, GET_ALL_PLAYERS, GET_SPECIFIC_PLAYER, ADD_TO_POT};
use crate::integer::{Integer, IntegerDeserialiser, SignedState};
use crate::ser_glue::string::{StringDeserialiser};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use crate::ser_glue::uuid::UuidDeserialiser;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum EventToServer {
    SendMessage {
        content: String
    },
    Introduction {
        name: String,
    },
    GetStartInformation,
    GetSpecificPlayer(Uuid),
    AddToPot(u32),
}

impl Serable for EventToServer {
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        match self {
            Self::SendMessage { content } => {
                into.push(TEXT_MESSAGE);
                content.ser_into(into);
            }
            Self::Introduction {name} => {
                into.push(INTRODUCTION);
                name.ser_into(into);
            }
            Self::GetStartInformation => {
                into.push(GET_ALL_PLAYERS);
            }
            Self::GetSpecificPlayer(uuid) => {
                into.push(GET_SPECIFIC_PLAYER);
                uuid.ser_into(into);
            }
            Self::AddToPot(n) => {
                into.push(ADD_TO_POT);
                Integer::from(*n).ser_into(into);
            }
        }
    }
}

impl Deserable for EventToServer {
    type Deserer = ServerEventDeserer;
}

#[derive(Debug)]
pub enum ServerEventDeserer {
    Start(u8),
    GotStart(u8),
    DeseringTxtMsg(StringDeserialiser),
    DeseringIntroduction(StringDeserialiser),
    DeseringAddToPot(IntegerDeserialiser),
    DeseringGetSpecificPlayer(UuidDeserialiser),
}

impl DeserMachine for ServerEventDeserer {
    type ExtraInput = ();
    type Output = EventToServer;
    type Error = EventReadError;

    fn new() -> Self {
        Self::Start(0)
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::Start(space) => DesiredInput::Byte(space),
            Self::GotStart(_start) => DesiredInput::ProcessMe,
            Self::DeseringTxtMsg(deser) | Self::DeseringIntroduction(deser) => deser.wants_read(),
            Self::DeseringAddToPot(deser) => deser.wants_read(),
            Self::DeseringGetSpecificPlayer(deser) => deser.wants_read(),
        }
    }

    fn give_starting_input(&mut self, (): Self::ExtraInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match self {
            Self::Start(start) => {
                if n == 1 {
                    *self = Self::GotStart(*start);
                }
            }
            Self::GotStart(_) => {},
            Self::DeseringTxtMsg(deser) | Self::DeseringIntroduction(deser) => deser.finish_bytes_for_writing(n),
            Self::DeseringAddToPot(deser) => deser.finish_bytes_for_writing(n),
            Self::DeseringGetSpecificPlayer(deser) => deser.finish_bytes_for_writing(n),
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::Start(n) => Ok(FsmResult::Continue(Self::Start(n))),
            Self::GotStart(n) => match n {
                TEXT_MESSAGE => Ok(FsmResult::Continue(Self::DeseringTxtMsg(String::deser()))),
                INTRODUCTION => Ok(FsmResult::Continue(Self::DeseringIntroduction(String::deser()))),
                GET_ALL_PLAYERS => Ok(FsmResult::Done(EventToServer::GetStartInformation)),
                GET_SPECIFIC_PLAYER => Ok(FsmResult::Continue(Self::DeseringGetSpecificPlayer(Uuid::deser()))),
                ADD_TO_POT => Ok(FsmResult::Continue(Self::DeseringAddToPot(Integer::deser_with_input(SignedState::Unsigned)))),
                n => Err(EventReadError::InvalidKind(n)),
            },
            Self::DeseringTxtMsg(deser) => {
                deser.mapped_process(Self::DeseringTxtMsg, |content| EventToServer::SendMessage {content})
            }
            Self::DeseringIntroduction(deser) => {
                deser.mapped_process(Self::DeseringIntroduction, |name| EventToServer::Introduction {name})
            }
            Self::DeseringAddToPot(deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::DeseringAddToPot(deser))),
                FsmResult::Done(amt) => {
                    let amt: u32 = amt.try_into()?;
                    Ok(FsmResult::Done(EventToServer::AddToPot(amt)))
                }
            }
            Self::DeseringGetSpecificPlayer(deser) => {
                Ok(deser.mapped_process::<_, _, std::convert::Infallible>(Self::DeseringGetSpecificPlayer, EventToServer::GetSpecificPlayer).unwrap())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use crate::events::server::{EventToServer, ServerEventDeserer};
    use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

    #[test]
    fn ser_events_individually() {
        for example in example_data() {
            eprintln!("testing: {example:#?}");
            let serialised = example.ser().1;
            let deserialised = deser_from_vec(serialised).unwrap();
            assert_eq!(example, deserialised[0]);
        }
    }

    #[test]
    fn ser_events_mass () {
        let example_data = example_data().to_vec();
        let mut output = vec![];
        example_data.iter().for_each(|e| e.ser_into(&mut output));
        let deserialised = deser_from_vec(output).unwrap();
        assert_eq!(example_data, deserialised);
    }

    fn example_data () -> [EventToServer; 5] {
        [
            EventToServer::SendMessage {content: "sup? 🤣🤣🤣".to_string()},
            EventToServer::Introduction {name: "範例名稱".to_string()},
            EventToServer::GetStartInformation,
            EventToServer::GetSpecificPlayer(Uuid::new_v4()),
            EventToServer::AddToPot(u32::MAX),
        ]
    }

    fn deser_from_vec (v: Vec<u8>) -> Result<Vec<EventToServer>, Box<dyn std::error::Error>> {
        let mut binary = v.into_iter().peekable();
        let mut deserer = EventToServer::deser();
        let mut found = vec![];

        loop {
            if binary.peek().is_none() && matches!(deserer, ServerEventDeserer::Start(_)) {
                break;
            }

            match deserer.wants_read() {
                DesiredInput::Byte(space) => {
                    if let Some(byte) = binary.next() {
                        *space = byte;
                        deserer.finish_bytes_for_writing(1);
                    }
                }
                DesiredInput::Bytes(space) => {
                    let mut n = 0;
                    for next_space in space {
                        let Some(byte) = binary.next() else {
                            break;
                        };
                        *next_space = byte;

                        n += 1;
                    }
                    deserer.finish_bytes_for_writing(n);
                }
                DesiredInput::ProcessMe => {
                    deserer = match deserer.process()? {
                        FsmResult::Continue(cont) => cont,
                        FsmResult::Done(evt) => {
                            found.push(evt);
                            EventToServer::deser()
                        }
                    }
                }
                DesiredInput::Extra => unreachable!()
            }
        }

        Ok(found)
    }
}