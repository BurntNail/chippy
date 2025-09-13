use crate::integer::{Integer, IntegerDeserialiser, IntegerReadError, SignedState};
use crate::ser_glue::string::{StringDeserialiser, StringReadError};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use std::fmt::{Display, Formatter};
use std::hash::Hash;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Player {
    pub name: String,
    pub balance: u32,
}

impl Display for Player {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.name)
    }
}

impl Serable for Player {
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        self.name.ser_into(into);
        Integer::from(self.balance).ser_into(into);
    }
}

#[derive(Debug)]
pub enum PlayerDeserialiser {
    GettingName(StringDeserialiser),
    GettingBalance {
        name: String,
        deser: IntegerDeserialiser,
    },
}

impl Deserable for Player {
    type Deserer = PlayerDeserialiser;
}

#[derive(Debug)]
pub enum PlayerReadError {
    String(StringReadError),
    Int(IntegerReadError),
}

impl From<StringReadError> for PlayerReadError {
    fn from(value: StringReadError) -> Self {
        Self::String(value)
    }
}
impl From<IntegerReadError> for PlayerReadError {
    fn from(value: IntegerReadError) -> Self {
        Self::Int(value)
    }
}

impl Display for PlayerReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(string) => write!(f, "Error deserialising name: {string}"),
            Self::Int(i) => write!(f, "Error deserialising balance: {i}"),
        }
    }
}

impl std::error::Error for PlayerReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::String(string) => Some(string),
            Self::Int(i) => Some(i),
        }
    }
}

impl DeserMachine for PlayerDeserialiser {
    type ExtraInput = ();
    type Output = Player;
    type Error = PlayerReadError;

    fn new() -> Self {
        Self::GettingName(StringDeserialiser::new())
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::GettingName(deser) => deser.wants_read(),
            Self::GettingBalance { deser, .. } => deser.wants_read(),
        }
    }

    fn give_starting_input(&mut self, (): Self::ExtraInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match self {
            Self::GettingName(deser) => deser.finish_bytes_for_writing(n),
            Self::GettingBalance { deser, .. } => deser.finish_bytes_for_writing(n),
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::GettingName(deser) => match deser.process()? {
                FsmResult::Continue(deser) => Ok(FsmResult::Continue(Self::GettingName(deser))),
                FsmResult::Done(name) => Ok(FsmResult::Continue(Self::GettingBalance {
                    name,
                    deser: Integer::deser_with_input(SignedState::Unsigned),
                })),
            },
            //allowed to unwrap because infallible
            Self::GettingBalance { name, deser } => match deser.process()? {
                FsmResult::Continue(deser) => {
                    Ok(FsmResult::Continue(Self::GettingBalance { name, deser }))
                }
                FsmResult::Done(balance) => Ok(FsmResult::Done(Player {
                    name,
                    balance: balance.try_into()?,
                })),
            },
        }
    }
}
