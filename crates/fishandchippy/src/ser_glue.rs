use std::fmt::Debug;

pub mod list;
pub mod map;
pub mod string;
pub mod tuple;
pub mod uuid;

pub trait Serable {
    type ExtraOutput;

    fn ser(&self) -> (Self::ExtraOutput, Vec<u8>) {
        let mut vec = vec![];
        let extra = self.ser_into(&mut vec);

        (extra, vec)
    }
    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput;
}

//ty amos: https://fasterthanli.me/articles/the-case-for-sans-io#the-structure-of-rc-zip
/// Indicates whether the state machine has completed its work
#[derive(Debug)]
pub enum FsmResult<M, R> {
    /// The I/O loop needs to continue, the state machine is given back.
    Continue(M),

    /// The state machine is done, and the result is returned.
    Done(R),
}

pub enum DesiredInput<'a> {
    Byte(&'a mut u8),
    Bytes(&'a mut [u8]),
    Extra,
    ProcessMe,
}

pub trait Deserable {
    type Deserer: DeserMachine<Output = Self>;

    #[must_use]
    fn deser() -> Self::Deserer {
        Self::Deserer::new()
    }
    #[must_use]
    fn deser_with_input(input: <Self::Deserer as DeserMachine>::ExtraInput) -> Self::Deserer {
        Self::Deserer::new_with_starting_input(input)
    }
}

pub trait DeserMachine: Sized {
    type ExtraInput;
    type Output;
    type Error: std::error::Error;

    fn new() -> Self;
    fn new_with_starting_input(input: Self::ExtraInput) -> Self {
        let mut s = Self::new();
        s.give_starting_input(input);
        s
    }
    fn wants_read(&mut self) -> DesiredInput<'_>;
    fn give_starting_input(&mut self, magic: Self::ExtraInput);
    fn finish_bytes_for_writing(&mut self, n: usize);
    #[allow(clippy::missing_errors_doc)] //you provide it lol i have no clue what the problem could be
    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error>;

    #[allow(clippy::missing_errors_doc)] //samesies
    fn mapped_process<MappedInput, MappedOutput, MappedError: From<Self::Error>>(
        self,
        continue_variant: impl FnOnce(Self) -> MappedInput,
        done_variant: impl FnOnce(Self::Output) -> MappedOutput,
    ) -> Result<FsmResult<MappedInput, MappedOutput>, MappedError> {
        match self.process()? {
            FsmResult::Continue(deser) => Ok(FsmResult::Continue(continue_variant(deser))),
            FsmResult::Done(done) => Ok(FsmResult::Done(done_variant(done))),
        }
    }
}
