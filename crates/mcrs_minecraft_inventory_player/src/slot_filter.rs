use mcrs_minecraft_inventory_transaction::Filter;

use crate::ArmourSlot;
use crate::EquipSlot;
use crate::Offer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotFilter {
    Any,
    Armour(ArmourSlot),
    CraftResult,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArmourLock {
    #[default]
    Free,
    Locked,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerMode {
    Creative,
    #[default]
    NotCreative,
}

impl SlotFilter {
    #[must_use]
    pub fn place(self, offer: Offer) -> Filter {
        let permits = match self {
            Self::Any => true,
            Self::Armour(slot) => offer.wear == Some(EquipSlot::Armour(slot)),
            Self::CraftResult => false,
        };
        if permits {
            Filter::Permits
        } else {
            Filter::Refuses
        }
    }

    #[must_use]
    pub fn take(self, lock: ArmourLock, mode: PlayerMode) -> Filter {
        match (self, lock, mode) {
            (Self::Armour(_), ArmourLock::Locked, PlayerMode::NotCreative) => Filter::Refuses,
            _ => Filter::Permits,
        }
    }
}
