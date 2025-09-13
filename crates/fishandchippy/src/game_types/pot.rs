use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use uuid::Uuid;
use crate::integer::{Integer, IntegerDeserialiser, IntegerReadError, SignedState};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use crate::ser_glue::list::{ListDeserialiser, ListSer};
use crate::ser_glue::tuple::{TupleDeserialiser, TupleReadError};
use crate::ser_glue::uuid::UuidDeserialiser;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Pot {
    pub current_value: u32,
    pub ready_to_put_in: HashMap<Uuid, u32>,
}


impl Serable for Pot {
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        Integer::from(self.current_value).ser_into(into);
        
        let to_put_in = 
            self.ready_to_put_in
                .iter()
                .map(|(uuid, value)| (*uuid, Integer::from(*value)))
                .collect::<Vec<_>>();
        
        Integer::from(to_put_in.len()).ser_into(into);
        ListSer(&to_put_in).ser_into(into); //can discard extras as we know they're constant
    }
}

#[derive(Debug)]
pub enum PotDeserialiser {
    ReadingPotSize(IntegerDeserialiser),
    ReadingNumberOfPlayers(u32, IntegerDeserialiser),
    ReadingSoFars(u32, ListDeserialiser<TupleDeserialiser<UuidDeserialiser, IntegerDeserialiser>>),
}

impl Deserable for Pot {
    type Deserer = PotDeserialiser;
}

#[derive(Debug)]
pub enum PotReadError {
    Integer(IntegerReadError),
    //TODO: is this really the best way of doing this?
    Player(TupleReadError<std::convert::Infallible, IntegerReadError>),
}
impl From<IntegerReadError> for PotReadError {
    fn from(value: IntegerReadError) -> Self {
        Self::Integer(value)
    }
}
impl From<TupleReadError<std::convert::Infallible, IntegerReadError>> for PotReadError {
    fn from(value: TupleReadError<std::convert::Infallible, IntegerReadError>) -> Self {
        Self::Player(value)
    }
}
impl Display for PotReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Integer(int) => write!(f, "Error parsing integer: {int}"),
            Self::Player(player) => write!(f, "Error parsing player: {player}"),
        }
    }
}
impl std::error::Error for PotReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Integer(int) => Some(int),
            Self::Player(player) => Some(player),
        }
    }
}

impl DeserMachine for PotDeserialiser {
    type ExtraInput = ();
    type Output = Pot;
    type Error = PotReadError;

    fn new() -> Self {
        Self::ReadingPotSize(Integer::deser_with_input(SignedState::Unsigned))
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::ReadingPotSize(deser) | Self::ReadingNumberOfPlayers(_, deser) => deser.wants_read(),
            Self::ReadingSoFars(_, deser) => deser.wants_read(),
        }
    }

    fn give_starting_input(&mut self, (): Self::ExtraInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match self {
            Self::ReadingPotSize(deser) | Self::ReadingNumberOfPlayers(_, deser) => deser.finish_bytes_for_writing(n),
            Self::ReadingSoFars(_, deser) => deser.finish_bytes_for_writing(n),
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::ReadingPotSize(deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::ReadingPotSize(deser))),
                FsmResult::Done(pot) => {
                    let pot = pot.try_into()?;
                    Ok(FsmResult::Continue(Self::ReadingNumberOfPlayers(pot, Integer::deser_with_input(SignedState::Unsigned))))
                }
            }
            Self::ReadingNumberOfPlayers(pot, deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::ReadingNumberOfPlayers(pot, deser))),
                FsmResult::Done(number_in_pot) => {
                    Ok(FsmResult::Continue(Self::ReadingSoFars(pot, ListDeserialiser::new_with_starting_input(vec![((), SignedState::Unsigned); number_in_pot.try_into()?]))))
                }
            }
            Self::ReadingSoFars(current_value, deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::ReadingSoFars(current_value, deser))),
                FsmResult::Done(list) => {
                    Ok(FsmResult::Done(Pot {
                        current_value,
                        ready_to_put_in: list
                            .into_iter()
                            .map(|(player, amt)| amt.try_into().map(|amt| (player, amt)))
                            .collect::<Result<HashMap<_, _>, _>>()?
                    }))
                }
            }
        }
    }
}