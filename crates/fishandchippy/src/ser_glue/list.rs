use crate::integer::{Integer, IntegerDeserialiser, IntegerReadError, SignedState};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use std::collections::VecDeque;
use std::fmt::{Debug, Display, Formatter};

pub struct BasicListSer<'a, T>(pub &'a [T]);
impl<T> Serable for BasicListSer<'_, T>
where
    T: Serable<ExtraOutput = ()>,
{
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        Integer::from(self.0.len()).ser_into(into);
        ListSer(self.0).ser_into(into);
    }
}

#[derive(Debug)]
pub enum BasicListReadError<E: std::error::Error> {
    Len(IntegerReadError),
    Element(E),
}

impl<E: std::error::Error> Display for BasicListReadError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Len(len) => write!(f, "Error getting len of list: {len}"),
            Self::Element(el) => write!(f, "Error getting list element: {el}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BasicListReadError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Len(len) => Some(len),
            Self::Element(el) => Some(el),
        }
    }
}

pub enum BasicListDeserialiser<D: DeserMachine<ExtraInput = ()>> {
    GettingLen(IntegerDeserialiser),
    GettingElements(ListDeserialiser<D>),
}

impl<D> DeserMachine for BasicListDeserialiser<D>
where
    D: DeserMachine<ExtraInput = ()>,
    <D as DeserMachine>::Error: 'static,
{
    type ExtraInput = ();
    type Output = Vec<D::Output>;
    type Error = BasicListReadError<D::Error>;

    fn new() -> Self {
        Self::GettingLen(Integer::deser_with_input(SignedState::Unsigned))
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::GettingLen(deser) => deser.wants_read(),
            Self::GettingElements(deser) => deser.wants_read(),
        }
    }

    fn give_starting_input(&mut self, (): Self::ExtraInput) {}

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match self {
            Self::GettingLen(deser) => deser.finish_bytes_for_writing(n),
            Self::GettingElements(deser) => deser.finish_bytes_for_writing(n),
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::GettingLen(deser) => match deser.process() {
                Err(e) => Err(BasicListReadError::Len(e)),
                Ok(FsmResult::Continue(deser)) => Ok(FsmResult::Continue(Self::GettingLen(deser))),
                Ok(FsmResult::Done(len)) => {
                    let len = match len.try_into() {
                        Ok(len) => len,
                        Err(e) => return Err(BasicListReadError::Len(e)),
                    };

                    Ok(FsmResult::Continue(Self::GettingElements(
                        ListDeserialiser::new_with_starting_input(vec![(); len]),
                    )))
                }
            },
            Self::GettingElements(deser) => deser
                .mapped_process(Self::GettingElements, std::convert::identity)
                .map_err(BasicListReadError::Element),
        }
    }
}

pub struct ListSer<'a, T>(pub &'a [T]);

impl<T: Serable> Serable for ListSer<'_, T> {
    type ExtraOutput = Vec<T::ExtraOutput>;

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        self.0.iter().map(|item| item.ser_into(into)).collect()
    }
}

#[derive(Debug)]
pub enum ListDeserialiser<D: DeserMachine> {
    AwaitingExtras,
    GettingElements {
        extras: VecDeque<D::ExtraInput>,
        so_far: Vec<D::Output>,
        current: D,
    },
    FoundEmptyExtras,
}

impl<D> DeserMachine for ListDeserialiser<D>
where
    D: DeserMachine,
    <D as DeserMachine>::Error: 'static,
{
    type ExtraInput = Vec<D::ExtraInput>;
    type Output = Vec<D::Output>;
    type Error = D::Error;

    fn new() -> Self {
        Self::AwaitingExtras
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::AwaitingExtras => DesiredInput::Extra,
            Self::GettingElements { current, .. } => current.wants_read(),
            Self::FoundEmptyExtras => DesiredInput::ProcessMe,
        }
    }

    fn give_starting_input(&mut self, extras: Self::ExtraInput) {
        if matches!(self, Self::AwaitingExtras) {
            let mut extras: VecDeque<D::ExtraInput> = extras.into();

            *self = extras.pop_front().map_or_else(
                || Self::FoundEmptyExtras,
                |first_extra| Self::GettingElements {
                    extras,
                    so_far: Vec::new(),
                    current: D::new_with_starting_input(first_extra),
                },
            );
        }
    }

    fn finish_bytes_for_writing(&mut self, n: usize) {
        match self {
            Self::GettingElements { current, .. } => current.finish_bytes_for_writing(n),
            Self::AwaitingExtras | Self::FoundEmptyExtras => {}
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::AwaitingExtras => Ok(FsmResult::Continue(Self::AwaitingExtras)),
            Self::GettingElements {
                mut extras,
                mut so_far,
                current,
            } => match current.process()? {
                FsmResult::Continue(current) => Ok(FsmResult::Continue(Self::GettingElements {
                    extras,
                    so_far,
                    current,
                })),
                FsmResult::Done(next_element) => {
                    so_far.push(next_element);
                    match extras.pop_front() {
                        Some(next_extra) => Ok(FsmResult::Continue(Self::GettingElements {
                            extras,
                            so_far,
                            current: D::new_with_starting_input(next_extra),
                        })),
                        None => Ok(FsmResult::Done(so_far)),
                    }
                }
            },
            Self::FoundEmptyExtras => Ok(FsmResult::Done(vec![])),
        }
    }
}

//TODO: above but for constant length arrays? maybe not useful enough...
