use mcrs_minecraft_resource::ResourceLocationBuf;
use std::fmt;

use serde::Serialize;

use crate::DialogRef;

/// The types a text names from outside it: `Self` is what a `show_item` hover
/// carries, and the associated types are the entity a `show_entity` hover and
/// the dialog a `show_dialog` click name. `()` names them by location alone.
pub trait TextTypes: Value {
    type EntityKey: Value;
    type Dialog: Value;
}

pub trait Value: Clone + PartialEq + fmt::Debug + Send + Sync + 'static {}

impl<T> Value for T where T: Clone + PartialEq + fmt::Debug + Send + Sync + 'static {}

pub trait Serde: Serialize + serde::de::DeserializeOwned {}

impl<T> Serde for T where T: Serialize + serde::de::DeserializeOwned {}

/// The text types that read and write themselves. Each implementation is
/// written out rather than blanket: an inline dialog holds text of the same
/// types, so a bound derived from the fields would prove itself in a cycle.
pub trait TextSerde: TextTypes<EntityKey: Serde, Dialog: Serde> + Serde {}

impl TextTypes for () {
    type EntityKey = ResourceLocationBuf;
    type Dialog = DialogRef;
}

impl TextSerde for () {}

/// Maps the types a text names from `A` to `B`, field by field.
pub trait TextMap<A: TextTypes, B: TextTypes> {
    type Error;

    fn item(&self, item: &A) -> Result<B, Self::Error>;

    fn entity(&self, entity: &A::EntityKey) -> Result<B::EntityKey, Self::Error>;

    fn dialog(&self, dialog: &A::Dialog) -> Result<B::Dialog, Self::Error>;

    fn at(error: Self::Error, field: &'static str) -> Self::Error;

    fn at_index(error: Self::Error, index: usize) -> Self::Error;
}
