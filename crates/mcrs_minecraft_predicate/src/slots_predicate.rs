use mcrs_minecraft_core::codec::unique_entries;
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_registries::{Named, Refs, Resolve, Resolved};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::slot_range::{SlotRange, UnknownSlotRange};

/// An item predicate for each slot range name; each entry needs one slot of its range.
#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) struct SlotsPredicate<M: Refs = Resolved>(Vec<SlotPredicate<M>>);

#[derive(Clone, Debug, PartialEq, Resolve)]
pub(crate) struct SlotPredicate<M: Refs = Resolved> {
    range: SlotRange,
    item: ItemPredicate<M>,
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
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
