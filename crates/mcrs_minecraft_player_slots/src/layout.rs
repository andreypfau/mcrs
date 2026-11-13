use core::ops::Range;

/// The number of slots of the player inventory.
pub const INVENTORY_SLOTS: u16 = 41;
/// The main slots of the player inventory; the hotbar is a part of them.
pub const MAIN: Range<u16> = 0..36;
/// The hotbar of the player inventory.
pub const HOTBAR: Range<u16> = 0..9;
/// The off-hand slot of the player inventory.
pub const OFF_HAND: u16 = 40;
/// The slot limit of each slot of a player that is not an armour slot.
pub(crate) const STACK_LIMIT: u8 = 99;
/// The slot limit of an armour slot.
pub(crate) const ARMOUR_LIMIT: u8 = 1;
