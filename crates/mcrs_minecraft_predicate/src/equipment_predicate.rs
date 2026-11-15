use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_registries::{Refs, Resolved, registry_value};
use serde::Deserialize;

/// An item predicate for each equipment slot of a living entity.
#[registry_value]
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub(crate) struct EquipmentPredicate<M: Refs = Resolved> {
    head: Option<ItemPredicate<M>>,
    chest: Option<ItemPredicate<M>>,
    legs: Option<ItemPredicate<M>>,
    feet: Option<ItemPredicate<M>>,
    body: Option<ItemPredicate<M>>,
    saddle: Option<ItemPredicate<M>>,
    mainhand: Option<ItemPredicate<M>>,
    offhand: Option<ItemPredicate<M>>,
}

impl EquipmentPredicate {
    /// `true` when no slot has an item predicate.
    pub(crate) fn is_empty(&self) -> bool {
        [
            &self.head,
            &self.chest,
            &self.legs,
            &self.feet,
            &self.body,
            &self.saddle,
            &self.mainhand,
            &self.offhand,
        ]
        .iter()
        .all(|slot| slot.is_none())
    }
}
