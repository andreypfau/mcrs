use crate::ArmourSlot;
use crate::INVENTORY_SLOTS;
use crate::OFF_HAND;
use crate::SlotFilter;
use crate::SlotRule;
use crate::layout::ARMOUR_LIMIT;
use crate::layout::STACK_LIMIT;

/// A holder of a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Holder {
    Inventory,
    Cursor,
    /// The 2 by 2 craft grid, in rows from the top left.
    CraftGrid,
    CraftResult,
}

impl Holder {
    /// The number of slots of this holder.
    #[must_use]
    pub const fn slot_count(self) -> u16 {
        match self {
            Self::Inventory => INVENTORY_SLOTS,
            Self::Cursor | Self::CraftResult => 1,
            Self::CraftGrid => 4,
        }
    }

    /// The slot limit and the filters of the slot `index`, or `None` when the holder has no such
    /// slot.
    #[must_use]
    pub fn slot(self, index: u16) -> Option<SlotRule> {
        if index >= self.slot_count() {
            return None;
        }
        let filter = match self {
            Self::Inventory if index == OFF_HAND => SlotFilter::OffHand,
            Self::Inventory => {
                ArmourSlot::from_index(index).map_or(SlotFilter::Any, SlotFilter::Armour)
            }
            Self::Cursor | Self::CraftGrid => SlotFilter::Any,
            Self::CraftResult => SlotFilter::CraftResult,
        };
        let limit = match filter {
            SlotFilter::Armour(_) => ARMOUR_LIMIT,
            SlotFilter::Any | SlotFilter::OffHand | SlotFilter::CraftResult => STACK_LIMIT,
        };
        Some(SlotRule { limit, filter })
    }
}
