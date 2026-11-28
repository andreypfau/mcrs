use mcrs_minecraft_core::codec::unique_entries;
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::{Named, Refs, Registries, Resolve, Resolved};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::baked_item_predicate::BakedItemPredicate;
use crate::entity_view::EntityView;
use crate::predicate_issue::PredicateIssue;
use crate::slot_range::{SlotRange, UnknownSlotRange};

#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) struct SlotsPredicate<M: Refs = Resolved>(Vec<SlotPredicate<M>>);

#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) struct SlotPredicate<M: Refs = Resolved> {
    range: SlotRange,
    item: ItemPredicate<M>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedSlotTest {
    range: SlotRange,
    item: BakedItemPredicate,
}

impl<'de> Deserialize<'de> for SlotsPredicate<Named> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let entries: Vec<(String, ItemPredicate<Named>)> = unique_entries(d)?;
        let mut slots = Vec::with_capacity(entries.len());
        for (name, item) in entries {
            let range = name.parse().map_err(|UnknownSlotRange| {
                D::Error::custom(format_args!("unknown slot range {name}"))
            })?;
            slots.push(SlotPredicate { range, item });
        }
        Ok(SlotsPredicate(slots))
    }
}

impl SlotsPredicate {
    pub(crate) fn bake<L: NameLookup>(
        &self,
        registries: &Registries,
        names: &L,
    ) -> Result<Box<[BakedSlotTest]>, PredicateIssue> {
        self.0
            .iter()
            .map(|slot| {
                Ok(BakedSlotTest {
                    range: slot.range,
                    item: BakedItemPredicate::bake(&slot.item, registries, names)?,
                })
            })
            .collect()
    }
}

impl BakedSlotTest {
    pub(crate) fn matches<W: EntityView>(
        &self,
        world: &W,
        entity: W::Entity,
        items: &ItemTable,
    ) -> bool {
        self.range.any_slot(|slot| {
            world
                .slot(entity, slot)
                .is_some_and(|stack| self.item.matches(stack, items))
        })
    }
}
