use crate::integer::{Integer, IntegerDeserialiser, SignedState};
use crate::ser_glue::list::BasicListReadError;
use crate::ser_glue::tuple::{TupleDeserialiser, TupleReadError};
use crate::ser_glue::{DeserMachine, Deserable, DesiredInput, FsmResult, Serable};
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::hash::Hash;

pub struct BasicMapSer<'a, K, V>(pub &'a HashMap<K, V>);
impl<K, V> Serable for BasicMapSer<'_, K, V>
where
    K: Serable<ExtraOutput = ()>,
    V: Serable<ExtraOutput = ()>,
{
    type ExtraOutput = ();

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        Integer::from(self.0.len()).ser_into(into);
        MapSer(self.0).ser_into(into);
    }
}

#[derive(Debug)]
pub enum BasicMapDeserialiser<KDeser, VDeser>
where
    KDeser: DeserMachine<ExtraInput = ()>,
    VDeser: DeserMachine<ExtraInput = ()>,
    KDeser::Output: Debug,
    VDeser::Output: Debug, //TODO: rm
{
    GettingLen(IntegerDeserialiser),
    GettingElements(MapDeserialiser<KDeser, VDeser>),
}

impl<KDeser, VDeser> DeserMachine for BasicMapDeserialiser<KDeser, VDeser>
where
    KDeser: DeserMachine<ExtraInput = ()>,
    VDeser: DeserMachine<ExtraInput = ()>,
    KDeser::Error: 'static,
    VDeser::Error: 'static,
    KDeser::Output: Eq + Hash + Debug,
    VDeser::Output: Hash + Debug, //TODO: rm debugs
{
    type ExtraInput = ();
    type Output = HashMap<KDeser::Output, VDeser::Output>;
    type Error = BasicListReadError<TupleReadError<KDeser::Error, VDeser::Error>>;

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
                Err(error) => Err(BasicListReadError::Len(error)),
                Ok(FsmResult::Continue(deser)) => Ok(FsmResult::Continue(Self::GettingLen(deser))),
                Ok(FsmResult::Done(len)) => {
                    let len = match len.try_into() {
                        Ok(len) => len,
                        Err(error) => return Err(BasicListReadError::Len(error)),
                    };
                    Ok(FsmResult::Continue(Self::GettingElements(
                        MapDeserialiser::new_with_starting_input(vec![((), ()); len]),
                    )))
                }
            },
            Self::GettingElements(deser) => deser
                .mapped_process(Self::GettingElements, std::convert::identity)
                .map_err(BasicListReadError::Element),
        }
    }
}

pub struct MapSer<'a, K, V>(&'a HashMap<K, V>);
impl<K: Serable, V: Serable> Serable for MapSer<'_, K, V> {
    type ExtraOutput = Vec<(K::ExtraOutput, V::ExtraOutput)>;

    fn ser_into(&self, into: &mut Vec<u8>) -> Self::ExtraOutput {
        self.0
            .iter()
            .map(|(k, v)| (k.ser_into(into), v.ser_into(into)))
            .collect()
    }
}

#[derive(Debug)]
pub enum MapDeserialiser<KDeser: DeserMachine, VDeser: DeserMachine>
//TODO: remove below at some point tm
where
    <KDeser as DeserMachine>::Output: Debug,
    <VDeser as DeserMachine>::Output: Debug,
{
    AwaitingExtras,
    ReadingList {
        extras: VecDeque<(KDeser::ExtraInput, VDeser::ExtraInput)>,
        so_far: HashMap<KDeser::Output, VDeser::Output>,
        current: TupleDeserialiser<KDeser, VDeser>,
    },
    FoundEmptyExtras,
}

impl<KDeser, VDeser> DeserMachine for MapDeserialiser<KDeser, VDeser>
where
    KDeser: DeserMachine,
    VDeser: DeserMachine,
    KDeser::Error: 'static,
    VDeser::Error: 'static,
    KDeser::Output: Eq + Hash + Debug,
    VDeser::Output: Hash + Debug, //TODO: rm debugs
{
    type ExtraInput = Vec<(KDeser::ExtraInput, VDeser::ExtraInput)>;
    type Output = HashMap<KDeser::Output, VDeser::Output>;
    type Error = TupleReadError<KDeser::Error, VDeser::Error>;

    fn new() -> Self {
        Self::AwaitingExtras
    }

    fn wants_read(&mut self) -> DesiredInput<'_> {
        match self {
            Self::AwaitingExtras => DesiredInput::Extra,
            Self::ReadingList { current, .. } => current.wants_read(),
            Self::FoundEmptyExtras => DesiredInput::ProcessMe,
        }
    }

    fn give_starting_input(&mut self, magic: Self::ExtraInput) {
        if matches!(self, Self::AwaitingExtras) {
            let mut extras: VecDeque<_> = magic.into();

            *self = extras.pop_front().map_or_else(
                || Self::FoundEmptyExtras,
                |first_extra| Self::ReadingList {
                    extras,
                    so_far: HashMap::new(),
                    current: TupleDeserialiser::new_with_starting_input(first_extra),
                },
            );
        }
    }

    fn finish_bytes_for_writing(&mut self, n: usize) {
        if let Self::ReadingList { current, .. } = self {
            current.finish_bytes_for_writing(n);
        }
    }

    fn process(self) -> Result<FsmResult<Self, Self::Output>, Self::Error> {
        match self {
            Self::AwaitingExtras => Ok(FsmResult::Continue(Self::AwaitingExtras)),
            Self::FoundEmptyExtras => Ok(FsmResult::Done(HashMap::new())),
            Self::ReadingList {
                mut extras,
                mut so_far,
                current,
            } => match current.process()? {
                FsmResult::Continue(current) => Ok(FsmResult::Continue(Self::ReadingList {
                    extras,
                    so_far,
                    current,
                })),
                FsmResult::Done((new_key, new_value)) => {
                    so_far.insert(new_key, new_value);
                    match extras.pop_front() {
                        Some(next_extra) => Ok(FsmResult::Continue(Self::ReadingList {
                            extras,
                            so_far,
                            current: TupleDeserialiser::new_with_starting_input(next_extra),
                        })),
                        None => Ok(FsmResult::Done(so_far)),
                    }
                }
            },
        }
    }
}
