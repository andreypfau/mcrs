use crate::keys::Item;
use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_core::validated;
use mcrs_minecraft_core::{Id, ResourceKey};
use mcrs_minecraft_registries::{
    IdSet, KeySet, Named, Refs, Resolved, WireId, WireSet, registry_value,
};
use serde::{Deserialize, Serialize};

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

listed_items!(KeySet: ResourceKey<Item>, IdSet: Id<Item>, WireSet: WireId<Item>);

impl<M: Refs> Validate for Ingredient<M>
where
    M::Set<Item>: ListedItems,
{
    fn validate(&self) -> Result<(), String> {
        self.0.listed().map_or(Ok(()), check_items)
    }
}
