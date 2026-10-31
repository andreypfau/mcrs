use crate::keys::Item;
use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_core::validated;
use mcrs_minecraft_registries::{
    IdSet, KeySet, Named, Refs, Resolved, Wire, WireId, WireSet, registry_value,
};
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};
use mcrs_minecraft_wire::{Decode, Encode, Error, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::stack::ItemRef;

validated!(Ingredient<Named>);

/// The items a slot accepts: one item, a list or a tag, never empty and never
/// air.
#[registry_value]
#[derive(mcrs_minecraft_registries::FromWire, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", transparent)]
pub struct Ingredient<M: Refs = Resolved>(pub M::Set<Item>);

fn check_items<T: ItemRef>(entries: &[T]) -> Result<(), String> {
    if entries.is_empty() {
        return Err("Ingredients can't be empty".into());
    }
    if entries.iter().any(ItemRef::is_air) {
        return Err("Ingredient can't contain air".into());
    }
    Ok(())
}

/// The items a set lists, or `None` for a tag.
pub trait ListedItems {
    type Entry: ItemRef;

    fn listed(&self) -> Option<&[Self::Entry]>;
}

macro_rules! listed_items {
    ($($set:ident: $entry:ty),* $(,)?) => {$(
        impl ListedItems for $set<Item> {
            type Entry = $entry;

            fn listed(&self) -> Option<&[$entry]> {
                match self {
                    $set::Tag(_) => None,
                    $set::List(entries) => Some(entries),
                }
            }
        }
    )*};
}

listed_items!(KeySet: ResourceKeyBuf<Item>, IdSet: Id<Item>, WireSet: WireId<Item>);

impl<M: Refs> Validate for Ingredient<M>
where
    M::Set<Item>: ListedItems,
{
    fn validate(&self) -> Result<(), String> {
        self.0.listed().map_or(Ok(()), check_items)
    }
}

impl Encode for Ingredient<Wire> {
    fn encode(&self, w: impl Write) -> Result<()> {
        self.validate().map_err(Error::invalid)?;
        self.0.encode(w)
    }
}

impl Decode<'_> for Ingredient<Wire> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let ingredient = Ingredient(Decode::decode(r)?);
        ingredient.validate().map_err(Error::invalid)?;
        Ok(ingredient)
    }
}
