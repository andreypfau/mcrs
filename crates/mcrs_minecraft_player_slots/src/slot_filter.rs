use mcrs_minecraft_inventory_plan::Filter;

use crate::ArmourSlot;
use crate::OffHandTag;
use crate::Offer;

/// The take and place filters of a slot of a player. They apply to a request, never to the
/// `set` operation or to a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotFilter {
    /// A slot that permits each take and each place.
    Any,
    Armour(ArmourSlot),
    OffHand,
    CraftResult,
}

/// The value of the game rule `mcrs:offhand_accepts_any_item`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OffHandRule {
    #[default]
    AnyItem,
    /// Only an item of the item tag `mcrs:offhand_allowed`.
    TaggedItems,
}

/// Whether the stack of a slot has an enchantment with the effect
/// `minecraft:prevent_armor_change`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArmourLock {
    #[default]
    Free,
    Locked,
}

/// The game mode of the player, as far as a filter reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerMode {
    Creative,
    #[default]
    NotCreative,
}

impl SlotFilter {
    /// The place filter for `offer` under the off-hand game rule `rule`.
    #[must_use]
    pub fn place(self, offer: Offer, rule: OffHandRule) -> Filter {
        let permits = match self {
            Self::Any => true,
            Self::Armour(slot) => offer.wear == Some(slot),
            Self::OffHand => match rule {
                OffHandRule::AnyItem => true,
                OffHandRule::TaggedItems => offer.off_hand_tag == OffHandTag::Member,
            },
            Self::CraftResult => false,
        };
        if permits {
            Filter::Permits
        } else {
            Filter::Refuses
        }
    }

    /// The take filter for a stack with the lock `lock`, taken by a player in `mode`.
    #[must_use]
    pub fn take(self, lock: ArmourLock, mode: PlayerMode) -> Filter {
        match (self, lock, mode) {
            (Self::Armour(_), ArmourLock::Locked, PlayerMode::NotCreative) => Filter::Refuses,
            _ => Filter::Permits,
        }
    }
}
