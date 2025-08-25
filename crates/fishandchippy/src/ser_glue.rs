pub mod string;

pub trait Serable {
    type ExtraOutput;
    
    fn ser (&self) -> (Self::ExtraOutput, Vec<u8>) {
        let mut vec = vec![];
        let extra = self.ser_into(&mut vec);

        (extra, vec)
    }
    fn ser_into (&self, into: &mut Vec<u8>) -> Self::ExtraOutput;
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
    Start,
    ProcessMe,
}

pub trait Deserable {
    type Deserer: DeserMachine<Output = Self>;
    
    fn deser () -> Self::Deserer {
        Self::Deserer::new()
    }
    fn deser_with_input (input: <Self::Deserer as DeserMachine>::StartingInput) -> Self::Deserer {
        Self::Deserer::new_with_starting_input(input)
    }
}

pub trait DeserMachine: Sized {
    type StartingInput;
    type Output;
    type Error;
    
    fn new () -> Self;
    fn new_with_starting_input (input: Self::StartingInput) -> Self;
    fn wants_read (&mut self) -> DesiredInput;
    fn give_starting_input(&mut self, magic: Self::StartingInput);
    fn finish_bytes_for_writing (&mut self, n: usize);
    fn process (self) -> Result<FsmResult<Self, Self::Output>, Self::Error>;
}