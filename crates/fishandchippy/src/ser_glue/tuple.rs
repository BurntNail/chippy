use crate::ser_glue::{DeserMachine, DesiredInput, FsmResult, Serable};
use std::fmt::{Display, Formatter};

impl<A: Serable, B: Serable> Serable for (A, B) {
    type ExtraOutput = (A::ExtraOutput, B::ExtraOutput);

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        let (a, b) = self;
        let a = a.ser_into(into);
        let b = b.ser_into(into);
        //NB: map's serialisation relies on this not changing :)
        (a, b)
    }
}

#[derive(Debug)]
pub enum TupleReadError<AError, BError> {
    AError(AError),
    BError(BError),
}

impl<A: Display, B: Display> Display for TupleReadError<A, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AError(a) => write!(f, "Error deserialising first part of tuple: {a}"),
            Self::BError(b) => write!(f, "Error deserialising second part of tuple: {b}"),
        }
    }
}

impl<A: std::error::Error + 'static, B: std::error::Error + 'static> std::error::Error
    for TupleReadError<A, B>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AError(e) => Some(e),
            Self::BError(b) => Some(b),
        }
    }
}

//no impl Deserable for (A, B) to allow alt implementations if wanted

#[derive(Debug)]
pub enum TupleDeserialiser<ADeser, BDeser>
where
    ADeser: DeserMachine,
    BDeser: DeserMachine,
{
    Empty,
    ReadingA {
        reading_a: ADeser,
        b_extra: BDeser::ExtraInput,
    },
    ReadingB {
        a: ADeser::Output,
        reading_b: BDeser,
    },
}

impl<ADeser, BDeser> DeserMachine for TupleDeserialiser<ADeser, BDeser>
where
    ADeser: DeserMachine,
    BDeser: DeserMachine,
    ADeser::Error: 'static,
    BDeser::Error: 'static,
{
    type ExtraInput = (ADeser::ExtraInput, BDeser::ExtraInput);
    type Output = (ADeser::Output, BDeser::Output);
    type Error = TupleReadError<ADeser::Error, BDeser::Error>;

    fn new() -> Self {
        Self::Empty
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::Empty => DesiredInput::Extra,
            Self::ReadingA { reading_a, .. } => reading_a.wants_read(),
            Self::ReadingB { reading_b, .. } => reading_b.wants_read(),
        }
    }

    fn give_starting_input(&mut self, (a_extra, b_extra): Self::ExtraInput) {
        if matches!(self, Self::Empty) {
            *self = Self::ReadingA {
                reading_a: ADeser::new_with_starting_input(a_extra),
                b_extra,
            }
        }
    }

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match self {
            Self::Empty => {}
            Self::ReadingA { reading_a, .. } => {
                reading_a.finish_bytes_for_writing(n);
            }
            Self::ReadingB { reading_b, .. } => {
                reading_b.finish_bytes_for_writing(n);
            }
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::ReadingA { reading_a, b_extra } => {
                //can't use ?s because of conflicting impls
                match reading_a.process() {
                    Ok(reading_a) => match reading_a {
                        FsmResult::Continue(reading_a) => {
                            Ok(FsmResult::Continue(Self::ReadingA { reading_a, b_extra }))
                        }
                        FsmResult::Done(a) => Ok(FsmResult::Continue(Self::ReadingB {
                            a,
                            reading_b: BDeser::new_with_starting_input(b_extra),
                        })),
                    },
                    Err(e) => Err(TupleReadError::AError(e)),
                }
            }
            Self::ReadingB { a, reading_b } => match reading_b.process() {
                Ok(reading_b) => match reading_b {
                    FsmResult::Continue(reading_b) => {
                        Ok(FsmResult::Continue(Self::ReadingB { a, reading_b }))
                    }
                    FsmResult::Done(b) => Ok(FsmResult::Done((a, b))),
                },
                Err(e) => Err(TupleReadError::BError(e)),
            },
            s @ Self::Empty => Ok(FsmResult::Continue(s)),
        }
    }
}
