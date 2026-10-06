use crate::keys::Item;
use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_core::validated;
use mcrs_minecraft_registry::HolderSet;
use serde::{Deserialize, Serialize};

validated!(Ingredient);

/// The items a slot accepts: one item, a list or a tag, never empty and never
/// air.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", transparent)]
pub struct Ingredient(pub HolderSet<Item>);

impl Validate for Ingredient {
    fn validate(&self) -> Result<(), String> {
        let entries = match &self.0 {
            HolderSet::Named(_) => return Ok(()),
            HolderSet::One(item) => std::slice::from_ref(item),
            HolderSet::List(items) => &items[..],
        };
        if entries.is_empty() {
            return Err("Ingredients can't be empty".into());
        }
        if entries.contains(&Item::Air.id()) {
            return Err("Ingredient can't contain air".into());
        }
        Ok(())
    }
}
