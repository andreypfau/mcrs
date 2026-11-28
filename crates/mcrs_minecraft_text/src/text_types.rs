use std::fmt;

use serde::Serialize;

pub trait TextTypes: Value {
    type EntityKey: Value;
    type Dialog: Value;
}

pub trait Value: Clone + PartialEq + fmt::Debug + Send + Sync + 'static {}

impl<T> Value for T where T: Clone + PartialEq + fmt::Debug + Send + Sync + 'static {}

pub trait Serde: Serialize + serde::de::DeserializeOwned {}

impl<T> Serde for T where T: Serialize + serde::de::DeserializeOwned {}

pub trait TextSerde: TextTypes<EntityKey: Serde, Dialog: Serde> + Serde {}

pub trait TextMap<A: TextTypes, B: TextTypes> {
    type Error;

    fn item(&self, item: &A) -> Result<B, Self::Error>;

    fn entity(&self, entity: &A::EntityKey) -> Result<B::EntityKey, Self::Error>;

    fn dialog(&self, dialog: &A::Dialog) -> Result<B::Dialog, Self::Error>;

    fn at(error: Self::Error, field: &'static str) -> Self::Error;

    fn at_index(error: Self::Error, index: usize) -> Self::Error;
}
