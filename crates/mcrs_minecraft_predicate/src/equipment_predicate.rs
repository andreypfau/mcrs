use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::{Refs, Registries, Resolved, registry_value};
use serde::Deserialize;

use crate::baked_item_predicate::BakedItemPredicate;
use crate::entity_view::EntityView;
use crate::predicate_issue::PredicateIssue;

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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedEquipmentTest {
    slot: EquipmentSlot,
    item: BakedItemPredicate,
}

impl EquipmentPredicate {
    pub(crate) fn bake<L: NameLookup>(
        &self,
        registries: &Registries,
        names: &L,
    ) -> Result<Box<[BakedEquipmentTest]>, PredicateIssue> {
        [
            (EquipmentSlot::Head, &self.head),
            (EquipmentSlot::Chest, &self.chest),
            (EquipmentSlot::Legs, &self.legs),
            (EquipmentSlot::Feet, &self.feet),
            (EquipmentSlot::Body, &self.body),
            (EquipmentSlot::Saddle, &self.saddle),
            (EquipmentSlot::MainHand, &self.mainhand),
            (EquipmentSlot::OffHand, &self.offhand),
        ]
        .into_iter()
        .filter_map(|(slot, item)| item.as_ref().map(|item| (slot, item)))
        .map(|(slot, item)| {
            Ok(BakedEquipmentTest {
                slot,
                item: BakedItemPredicate::bake(item, registries, names)?,
            })
        })
        .collect()
    }
}

impl BakedEquipmentTest {
    pub(crate) fn matches<W: EntityView>(
        &self,
        world: &W,
        entity: W::Entity,
        items: &ItemTable,
    ) -> bool {
        world
            .equipment(entity, self.slot)
            .is_some_and(|stack| self.item.matches(stack, items))
    }
}
