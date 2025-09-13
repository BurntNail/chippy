use crate::game_types::player::PlayerReadError;
use crate::game_types::pot::PotReadError;
use crate::integer::IntegerReadError;
use crate::ser_glue::list::BasicListReadError;
use crate::ser_glue::string::StringReadError;
use crate::ser_glue::tuple::TupleReadError;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::string::FromUtf8Error;

pub mod client;
pub mod server;

const TEXT_MESSAGE: u8 = 1;
const ADMIN_MSG: u8 = 2;
const INTRODUCTION: u8 = 3;
const ADD_TO_POT: u8 = 10;
const GET_POT: u8 = 11;
const GET_ALL_PLAYERS: u8 = 20;
const GET_SPECIFIC_PLAYER: u8 = 21;

#[derive(Debug)]
pub enum EventReadError {
    InvalidString(FromUtf8Error),
    Integer(IntegerReadError),
    InvalidKind(u8),
    StringRead(StringReadError),
    Pot(PotReadError),
    ListOfPlayers(BasicListReadError<TupleReadError<Infallible, PlayerReadError>>),
    Player(PlayerReadError),
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
impl From<PotReadError> for EventReadError {
    fn from(value: PotReadError) -> Self {
        Self::Pot(value)
    }
}
impl From<BasicListReadError<TupleReadError<Infallible, PlayerReadError>>> for EventReadError {
    fn from(value: BasicListReadError<TupleReadError<Infallible, PlayerReadError>>) -> Self {
        Self::ListOfPlayers(value)
    }
}
impl From<PlayerReadError> for EventReadError {
    fn from(value: PlayerReadError) -> Self {
        Self::Player(value)
    }
}

impl Display for EventReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidString(str) => write!(f, "Error reading UTF-8: {str}"),
            Self::Integer(int) => write!(f, "Error parsing integer value: {int}"),
            Self::InvalidKind(kind) => write!(f, "Invalid event type provided: {kind}"),
            Self::StringRead(str) => write!(f, "Error reading basic string: {str}"),
            Self::Pot(pot) => write!(f, "Error reading pot: {pot}"),
            Self::ListOfPlayers(players) => write!(f, "Error reading list of players: {players}"),
            Self::Player(player) => write!(f, "Error reading specific player: {player}"),
        }
    }
}

impl std::error::Error for EventReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidString(str) => Some(str),
            Self::Integer(int) => Some(int),
            Self::StringRead(str) => Some(str),
            Self::Pot(pot) => Some(pot),
            Self::ListOfPlayers(lop) => Some(lop),
            Self::Player(player) => Some(player),
            Self::InvalidKind(_) => None,
        }
    }
}
