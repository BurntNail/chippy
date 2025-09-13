use std::collections::HashMap;
use uuid::Uuid;
use crate::events::{EventReadError, ADMIN_MSG, GET_ALL_PLAYERS, GET_POT, INTRODUCTION, GET_SPECIFIC_PLAYER, TEXT_MESSAGE};
use crate::game_types::player::{Player, PlayerDeserialiser};
use crate::game_types::pot::{Pot, PotDeserialiser};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use crate::ser_glue::map::{BasicMapDeserialiser, BasicMapSer};
use crate::ser_glue::string::StringDeserialiser;
use crate::ser_glue::uuid::UuidDeserialiser;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventToClient {
    TxtSent(Uuid, String),
    AdminMsg(String),
    Introduced(Uuid),
    Pot(Pot),
    AllPlayers(HashMap<Uuid, Player>),
    SpecificPlayer(Uuid, Player),
}

impl Serable for EventToClient {
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        match self {
            Self::TxtSent(uuid, content) => {
                into.push(TEXT_MESSAGE);
                (*uuid, content.as_str()).ser_into(into);
            }
            Self::AdminMsg(content) => {
                into.push(ADMIN_MSG);
                content.ser_into(into);
            }
            Self::Introduced(uuid) => {
                into.push(INTRODUCTION);
                uuid.ser_into(into);
            }
            Self::Pot(pot) => {
                into.push(GET_POT);
                pot.ser_into(into);
            }
            Self::AllPlayers(players) => {
                into.push(GET_ALL_PLAYERS);
                BasicMapSer(players).ser_into(into);
            }
            Self::SpecificPlayer(uuid, player) => {
                into.push(GET_SPECIFIC_PLAYER);
                uuid.ser_into(into);
                player.ser_into(into);
            }
        }
    }
}

impl Deserable for EventToClient {
    type Deserer = ClientEventDeserer;
}

#[derive(Debug)]
pub enum ClientEventDeserer {
    Start(u8),
    GotStart(u8),
    DeseringTextUuid(UuidDeserialiser),
    DeseringTextAfterUuid(Uuid, StringDeserialiser),
    DeseringIntro(UuidDeserialiser),
    DeseringAdminMsg(StringDeserialiser),
    DeseringPot(PotDeserialiser),
    DeseringAllPlayers(BasicMapDeserialiser<UuidDeserialiser, PlayerDeserialiser>),
    DeseringPlayerUuid(UuidDeserialiser),
    DeseringPlayerAfterUuid(Uuid, PlayerDeserialiser)
}

impl DeserMachine for ClientEventDeserer {
    type ExtraInput = ();
    type Output = EventToClient;
    type Error = EventReadError;

    fn new() -> Self {
        Self::Start(0)
    }
    
    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::Start(space) => DesiredInput::Byte(space),
            Self::GotStart(_start) => DesiredInput::ProcessMe,
            Self::DeseringIntro(deser) | Self::DeseringTextUuid(deser) | Self::DeseringPlayerUuid(deser) => deser.wants_read(),
            Self::DeseringTextAfterUuid(_, deser) | Self::DeseringAdminMsg(deser) => deser.wants_read(),
            Self::DeseringPlayerAfterUuid(_, deser) => deser.wants_read(),
            Self::DeseringPot(deser) => deser.wants_read(),
            Self::DeseringAllPlayers(deser) => deser.wants_read(),
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
            Self::GotStart(_) => {}
            Self::DeseringIntro(deser) | Self::DeseringTextUuid(deser) | Self::DeseringPlayerUuid(deser) => deser.finish_bytes_for_writing(n),
            Self::DeseringTextAfterUuid(_, deser) | Self::DeseringAdminMsg(deser) => deser.finish_bytes_for_writing(n),
            Self::DeseringPlayerAfterUuid(_, deser) => deser.finish_bytes_for_writing(n),
            Self::DeseringPot(deser) => deser.finish_bytes_for_writing(n),
            Self::DeseringAllPlayers(deser) => deser.finish_bytes_for_writing(n),
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::Start(n) => Ok(FsmResult::Continue(Self::Start(n))),
            Self::GotStart(n) => match n {
                TEXT_MESSAGE => Ok(FsmResult::Continue(Self::DeseringTextUuid(Uuid::deser()))),
                INTRODUCTION => Ok(FsmResult::Continue(Self::DeseringIntro(Uuid::deser()))),
                ADMIN_MSG => Ok(FsmResult::Continue(Self::DeseringAdminMsg(String::deser()))),
                GET_POT => Ok(FsmResult::Continue(Self::DeseringPot(Pot::deser()))),
                GET_ALL_PLAYERS => Ok(FsmResult::Continue(Self::DeseringAllPlayers(BasicMapDeserialiser::new()))),
                GET_SPECIFIC_PLAYER => Ok(FsmResult::Continue(Self::DeseringPlayerUuid(Uuid::deser()))),
                n => Err(EventReadError::InvalidKind(n)),
            },
            Self::DeseringIntro(deser) => {
                Ok(deser.mapped_process::<_, _, std::convert::Infallible>(Self::DeseringIntro, EventToClient::Introduced).unwrap())
            }
            Self::DeseringTextUuid(deser) => match deser.process().unwrap() {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::DeseringTextUuid(deser))),
                FsmResult::Done(uuid) => Ok(FsmResult::Continue(Self::DeseringTextAfterUuid(uuid, String::deser()))),
            }
            Self::DeseringTextAfterUuid(uuid, deser) => {
                deser.mapped_process(
                    |deser| Self::DeseringTextAfterUuid(uuid, deser),
                    |msg| EventToClient::TxtSent(uuid, msg),
                )
            }
            Self::DeseringAdminMsg(deser) => {
                deser.mapped_process(Self::DeseringAdminMsg, EventToClient::AdminMsg)
            }
            Self::DeseringPot(deser) => {
                deser.mapped_process(Self::DeseringPot, EventToClient::Pot)
            }
            Self::DeseringAllPlayers(deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::DeseringAllPlayers(deser))),
                FsmResult::Done(players) => Ok(FsmResult::Done(EventToClient::AllPlayers(players))),
            },
            Self::DeseringPlayerUuid(deser) => match deser.process().unwrap() {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::DeseringPlayerUuid(deser))),
                FsmResult::Done(uuid) => Ok(FsmResult::Continue(Self::DeseringPlayerAfterUuid(uuid, Player::deser()))),
            }
            Self::DeseringPlayerAfterUuid(uuid, deser) => {
                deser.mapped_process(
                    |deser| Self::DeseringPlayerAfterUuid(uuid, deser),
                    |player| EventToClient::SpecificPlayer(uuid, player),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use uuid::Uuid;
    use crate::events::client::{ClientEventDeserer, EventToClient};
    use crate::game_types::player::Player;
    use crate::game_types::pot::Pot;
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

    fn example_data () -> [EventToClient; 6] {
        [
            EventToClient::TxtSent(Uuid::new_v4(), "argghhhhhhhhh éà🤧🤧🤧".to_string()),
            EventToClient::AdminMsg("get den'd ;)".to_string()),
            EventToClient::Introduced(Uuid::new_v4()),
            EventToClient::Pot(Pot {
                current_value: 123_456_789,
                ready_to_put_in: HashMap::from([(Uuid::new_v4(), 123), (Uuid::new_v4(), 456), (Uuid::new_v4(), 789)]),
            }),
            EventToClient::AllPlayers(HashMap::from([
                (Uuid::new_v4(), Player { name: "Alice".to_string(), balance: 1 }),
                (Uuid::new_v4(), Player { name: "François".to_string(), balance: u32::MAX }),
                (Uuid::new_v4(), Player { name: "範例名稱".to_string(), balance: u32::MAX - 1 })
            ])),
            EventToClient::SpecificPlayer(Uuid::new_v4(), Player {name: "".to_string(), balance: 0}),
        ]
    }
    
    fn deser_from_vec (v: Vec<u8>) -> Result<Vec<EventToClient>, Box<dyn std::error::Error>> {
        let mut binary = v.into_iter().peekable();
        let mut deserer = EventToClient::deser();
        let mut found = vec![];
        
        loop {
            if binary.peek().is_none() && matches!(deserer, ClientEventDeserer::Start(_)) {
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
                            EventToClient::deser()
                        }
                    }
                }
                DesiredInput::Extra => unreachable!()
            }
        }
        
        Ok(found)
    }
}