//! The slots of a player without Bevy: the holders and the indices of their slots, the slot
//! limits, the take and place filters, and the insert rule into the player inventory.
#![no_std]

mod armour_slot;
mod holder;
mod insert;
mod layout;
mod offer;
mod slot_filter;
mod slot_rule;
mod stacking;

pub use armour_slot::ArmourSlot;
pub use holder::Holder;
pub use insert::Insertion;
pub use insert::Slot;
pub use insert::insert;
pub use layout::HOTBAR;
pub use layout::INVENTORY_SLOTS;
pub use layout::MAIN;
pub use layout::OFF_HAND;
pub use offer::OffHandTag;
pub use offer::Offer;
pub use slot_filter::ArmourLock;
pub use slot_filter::OffHandRule;
pub use slot_filter::PlayerMode;
pub use slot_filter::SlotFilter;
pub use slot_rule::SlotRule;
pub use stacking::Stacking;
