use crate::keys::Item;
use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_core::validated;
use mcrs_minecraft_registries::{IdSet, KeySet, Named, Refs, Resolved, registry_value};
use serde::{Deserialize, Serialize};

use crate::stack::ItemRef;

validated!(Ingredient<Named>);

/// The items a slot accepts: one item, a list or a tag, never empty and never
/// air.
#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

impl Validate for Ingredient<Named> {
    fn validate(&self) -> Result<(), String> {
        match &self.0 {
            KeySet::Tag(_) => Ok(()),
            KeySet::List(entries) => check_items(entries),
        }
    }
}

impl Validate for Ingredient {
    fn validate(&self) -> Result<(), String> {
        match &self.0 {
            IdSet::Tag(_) => Ok(()),
            IdSet::List(entries) => check_items(entries),
        }
    }
}
