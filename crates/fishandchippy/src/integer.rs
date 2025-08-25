//! A module containing a struct [`Integer`] designed to minimise size when serialised.

use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::num::ParseIntError;
use crate::{display_bytes_as_hex_array};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};

///This represents whether a number is signed or unsigned. There are conversions to/from [`u8`]s which use two bytes.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum SignedState {
    #[allow(missing_docs)]
    Unsigned,
    SignedPositive,
    SignedNegative,
}

impl From<SignedState> for u8 {
    fn from(value: SignedState) -> Self {
        match value {
            SignedState::Unsigned => 0,
            SignedState::SignedPositive => 1,
            SignedState::SignedNegative => 2,
        }
    }
}
impl TryFrom<u8> for SignedState {
    type Error = IntegerReadError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unsigned),
            1 => Ok(Self::SignedPositive),
            2 => Ok(Self::SignedNegative),
            _ => Err(IntegerReadError::InvalidSignedStateDiscriminant(value)),
        }
    }
}

///The largest unsigned integer that can be stored using [`Integer`].
pub type BiggestInt = u128;
///The largest signed integer that can be stored using [`Integer`].
pub type BiggestIntButSigned = i128; //convenience so it's all at the top of the file
///The number of bytes required for storing one [`BiggestInt`]
const INTEGER_MAX_SIZE: usize = (BiggestInt::BITS / 8) as usize; //yes, I could >> 3, but it gets compile-time evaluated anyways and this is clearer
///The maximum size for an integer to be stored without a size before it
#[allow(clippy::cast_possible_truncation)]
pub const ONE_BYTE_MAX_SIZE: u8 = u8::MAX - (INTEGER_MAX_SIZE as u8);

///A type that represents an integer designed to be the smallest when serialised.
///
/// To create an `Integer`, there are many `From` implementations for every integer type in the standard library. To get a type out, there are many `TryFrom` implementations for those same integers. These are `TryFrom` as the stored content could be too large or be have a sign and not be able to be represented by an unsigned integer.
///
/// When converting to a floating point number, precision can be lost. When converting from a floating number, it can fail if:
/// - The floating point number was too large.
/// - The floating point number had a decimal part (currently checked using [`f64::fract`], [`f64::EPSILON`] and the [`f32`] equivalents).
#[derive(Copy, Clone)]
pub struct Integer {
    signed_state: SignedState,
    ///bytes - follows the signed-ness of `signed_state`
    content: [u8; INTEGER_MAX_SIZE],
    number_of_bytes_used: usize,
}

impl PartialEq for Integer {
    fn eq(&self, other: &Self) -> bool {
        if self.content[0..self.number_of_bytes_used]
            != other.content[0..other.number_of_bytes_used]
        {
            return false;
        }

        match self.signed_state {
            SignedState::Unsigned | SignedState::SignedPositive => {
                other.signed_state != SignedState::SignedNegative
            }
            SignedState::SignedNegative => other.signed_state == SignedState::SignedNegative,
        }
    }
}
impl Eq for Integer {}

impl Hash for Integer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let ss_to_be_hashed = if self.signed_state == SignedState::SignedNegative {
            SignedState::SignedNegative
        } else {
            SignedState::SignedPositive
        };
        ss_to_be_hashed.hash(state);
        self.content[0..self.number_of_bytes_used].hash(state);
        self.number_of_bytes_used.hash(state);
    }
}

impl Integer {
    ///Whether the number is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.signed_state == SignedState::SignedNegative
    }

    ///Whether the number is positive.
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.signed_state != SignedState::SignedNegative
    }
}

impl Display for Integer {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self.signed_state {
            SignedState::SignedPositive | SignedState::SignedNegative => {
                match BiggestIntButSigned::try_from(*self) {
                    Ok(i) => write!(f, "{i}"),
                    Err(e) => write!(f, "{e}"),
                }
            }
            SignedState::Unsigned => match BiggestInt::try_from(*self) {
                Ok(i) => write!(f, "{i}"),
                Err(e) => write!(f, "{e}"),
            },
        }
    }
}

#[allow(clippy::missing_fields_in_debug)]
impl Debug for Integer {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let hex = display_bytes_as_hex_array(&self.content);
        let content = self.to_string();

        f.debug_struct("Integer")
            .field("signed_state", &self.signed_state)
            .field("bytes_used", &self.number_of_bytes_used)
            .field("content", &content)
            .field("bytes", &hex)
            .finish()
    }
}

macro_rules! new_x {
    ($($t:ty => $name:ident),+) => {
        impl Integer {
            $(
                ///Creates an `Integer`
                #[must_use]
                pub fn $name(n: $t) -> Self {
                    <Self as From<$t>>::from(n)
                }
            )+
        }
    };
}

macro_rules! from_signed {
    ($($t:ty),+) => {
        $(
        impl From<$t> for Integer {
            fn from(n: $t) -> Self {
                if n == 0 {
                    Self {
                        signed_state: SignedState::SignedPositive,
                        content: [0; INTEGER_MAX_SIZE],
                        number_of_bytes_used: 0,
                    }
                } else if n == -1 {
                    Self {
                        signed_state: SignedState::SignedNegative,
                        content: [u8::MAX; INTEGER_MAX_SIZE],
                        number_of_bytes_used: 1,
                    }
                } else if n < 0 {
                    let mut content = [u8::MAX; INTEGER_MAX_SIZE];
                    let mut last_non_filled_byte = 0;

                    for (i, b) in n.to_le_bytes().into_iter().enumerate() {
                        content[i] = b;

                        if b != u8::MAX {
                            last_non_filled_byte = i + 1;
                        }
                    }

                    Self {
                        signed_state: SignedState::SignedNegative,
                        content,
                        number_of_bytes_used: last_non_filled_byte,
                    }
                } else {
                    let mut content = [0; INTEGER_MAX_SIZE];
                    let mut last_non_zero_byte = 0;

                    for (i, b) in n.to_le_bytes().into_iter().enumerate() {
                        content[i] = b;
                        if b != 0 {
                            last_non_zero_byte = i + 1;
                        }
                    }

                    Self {
                        signed_state: SignedState::SignedPositive,
                        content,
                        number_of_bytes_used: last_non_zero_byte,
                    }
                }
            }
        }

        impl TryFrom<Integer> for $t {
            type Error = IntegerReadError;

            fn try_from(i: Integer) -> Result<Self, Self::Error> {
                const T_BYTES: usize = (<$t>::BITS / 8) as usize;
                if i.number_of_bytes_used > T_BYTES {
                    return Err(IntegerReadError::TooBigToFit);
                }

                let out = if i.signed_state == SignedState::SignedNegative {
                    let mut start = [u8::MAX; T_BYTES];

                    for (i, b) in i.content
                        .into_iter()
                        .enumerate()
                        .take(i.number_of_bytes_used)
                    {
                        start[i] = b;
                    }

                    start
                } else {
                    let mut start = [0; T_BYTES];

                    for (i, b) in i
                        .content
                        .into_iter()
                        .enumerate()
                        .take(i.number_of_bytes_used)
                    {
                        start[i] = b;
                    }


                    start
                };

                Ok(<$t>::from_le_bytes(out))
            }
        }
        )+
    };
}
macro_rules! from_unsigned {
    ($($t:ty),+) => {
        $(
        impl From<$t> for Integer {
            fn from(n: $t) -> Self {
                let mut content = [0_u8; INTEGER_MAX_SIZE];
                let mut last_non_zero_byte = 0;
                for (i, b) in n.to_le_bytes().into_iter().enumerate() {
                    content[i] = b;
                    if b != 0 {
                        last_non_zero_byte = i;
                    }
                }

                Self {
                    signed_state: SignedState::Unsigned,
                    content,
                    number_of_bytes_used: last_non_zero_byte + 1
                }
            }
        }
        impl TryFrom<Integer> for $t {
            type Error = IntegerReadError;

            fn try_from(i: Integer) -> Result<Self, Self::Error> {
                const T_BYTES: usize = (<$t>::BITS / 8) as usize;
                if i.number_of_bytes_used > T_BYTES {
                    return Err(IntegerReadError::TooBigToFit);
                }
                if i.signed_state == SignedState::SignedNegative {
                    return Err(IntegerReadError::SignError);
                }

                let mut out = [0_u8; T_BYTES];
                for (i, b) in i
                    .content
                    .into_iter()
                    .enumerate()
                    .take(T_BYTES)
                {
                    out[i] = b;
                }

                Ok(<$t>::from_le_bytes(out))

            }
        }
        )+
    };
}

new_x!(u8 => u8, i8 => i8, u16 => u16, i16 => i16, u32 => u32, i32 => i32, usize => usize, isize => isize, u64 => u64, i64 => i64, u128 => u128, i128 => i128);

from_signed!(i8, i16, i32, i64, isize, i128);
from_unsigned!(u8, u16, u32, u64, usize, u128);

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
///Error type for dealing with serialisation errors related to [`Integer`]s.
pub enum IntegerReadError {
    ///An invalid signed state was found - these should only be `0b1` and `0b0`
    InvalidSignedStateDiscriminant(u8),
    ///Not enough bytes were within the cursor to deserialise the integer
    NotEnoughBytes,
    ///Integers can only be turned back into rust integers that they actually fit inside.
    TooBigToFit,
    ///Integers can only be turned back to their original sign
    SignError,
}

impl Display for IntegerReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            IntegerReadError::InvalidSignedStateDiscriminant(b) => {
                write!(f, "Invalid signed state discriminant found: {b:#b}")
            }
            IntegerReadError::NotEnoughBytes => write!(f, "Not enough bytes provided"),
            IntegerReadError::TooBigToFit => {
                write!(f, "Attempted to deserialise into size too small to fit")
            }
            IntegerReadError::SignError => write!(f, "Tried to fit integer into incorrect sign"),
        }
    }
}

impl std::error::Error for IntegerReadError {}

impl Serable for Integer {
    type ExtraOutput = SignedState;

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        if self.number_of_bytes_used <= 1 {
            let first_byte = self.content[0];
            if first_byte <= ONE_BYTE_MAX_SIZE {
                into.push(first_byte);
                return self.signed_state;
            }
        }

        let stored_size = self.number_of_bytes_used;
        let bytes = self.content;

        let size = ONE_BYTE_MAX_SIZE + stored_size as u8;

        into.push(size);
        if stored_size != 0 {
            into.extend(&bytes[0..stored_size]);
        }

        self.signed_state
    }
}

impl Deserable for Integer {
    type Deserer = IntegerDeserialiser;
}

pub enum IntegerDeserialiser {
    Start,
    GotSignedState {
        state: SignedState,
        to_be_first_byte: u8,
    },
    GotSignedStateAndFirstByte {
        state: SignedState,
        first_byte: u8
    },
    GotLenAndSomeBytes {
        state: SignedState,
        so_far: usize,
        space: Vec<u8>,
    },
    GotAllBytes(SignedState, Vec<u8>),
}

impl DeserMachine for IntegerDeserialiser {
    type StartingInput = SignedState;
    type Output = Integer;
    type Error = IntegerReadError;

    fn new() -> Self {
        Self::Start
    }

    fn new_with_starting_input(state: Self::StartingInput) -> Self {
        Self::GotSignedState {
            state, to_be_first_byte: 0
        }
    }

    fn wants_read(&mut self) -> DesiredInput {
        match self {
            Self::Start => DesiredInput::Start, 
            Self::GotSignedState {
                state: _, to_be_first_byte
            } => DesiredInput::Byte(to_be_first_byte),
            Self::GotSignedStateAndFirstByte {..} => DesiredInput::ProcessMe,
            Self::GotLenAndSomeBytes {
                state: _,
                so_far,
                space
            } => DesiredInput::Bytes(&mut space[*so_far..]),
            Self::GotAllBytes(_, _) => DesiredInput::ProcessMe
        }
    }

    fn give_starting_input(&mut self, state: Self::StartingInput) {
        if matches!(self, Self::Start) {
            *self = Self::GotSignedState {
                state,
                to_be_first_byte: 0
            };
        }
    }

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match core::mem::replace(self, Self::Start) {
            IntegerDeserialiser::Start => {}
            IntegerDeserialiser::GotSignedState { state, to_be_first_byte } => {
                *self = IntegerDeserialiser::GotSignedStateAndFirstByte {state, first_byte: to_be_first_byte};
            }
            IntegerDeserialiser::GotSignedStateAndFirstByte { state, first_byte } => {
                *self = IntegerDeserialiser::GotSignedStateAndFirstByte {state, first_byte};
            }
            IntegerDeserialiser::GotLenAndSomeBytes {
                state, so_far, space
            } => {
                *self = if so_far + n == space.len() {
                    IntegerDeserialiser::GotAllBytes(state, space)
                } else {
                    IntegerDeserialiser::GotLenAndSomeBytes {
                        state,
                        so_far: so_far + n,
                        space,
                    }
                };
            }
            IntegerDeserialiser::GotAllBytes(state, bytes) => {
                *self = IntegerDeserialiser::GotAllBytes(state, bytes);
            }
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            IntegerDeserialiser::GotSignedStateAndFirstByte { state, first_byte } => {
                if first_byte <= ONE_BYTE_MAX_SIZE {
                    let mut content = if state == SignedState::SignedNegative {
                        [u8::MAX; INTEGER_MAX_SIZE]
                    } else {
                        [0; INTEGER_MAX_SIZE]
                    };
                    content[0] = first_byte;
                    let number_of_bytes_used =
                        usize::from(state == SignedState::SignedNegative || content[0] != 0);

                    Ok(FsmResult::Done(Integer {
                        signed_state: state,
                        content,
                        number_of_bytes_used,
                    }))
                } else {
                    Ok(FsmResult::Continue(IntegerDeserialiser::GotLenAndSomeBytes {
                        state,
                        so_far: 0,
                        space: vec![0; (first_byte - ONE_BYTE_MAX_SIZE) as usize],
                    }))
                }
            }
            IntegerDeserialiser::GotAllBytes(signed_state, bytes_stored) => {
                let mut content = if signed_state == SignedState::SignedNegative {
                    [u8::MAX; INTEGER_MAX_SIZE]
                } else {
                    [0; INTEGER_MAX_SIZE]
                };
                for (i, b) in bytes_stored.iter().copied().enumerate() {
                    content[i] = b;
                }

                Ok(FsmResult::Done( Integer{
                    signed_state,
                    content,
                    number_of_bytes_used: bytes_stored.len(),
                }))
            }
            waiting_for_data_states => {
                Ok(FsmResult::Continue(waiting_for_data_states))
            }
        }
    }
}