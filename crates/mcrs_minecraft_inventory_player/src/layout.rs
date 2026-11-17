use core::ops::Range;

pub const INVENTORY_SLOTS: u16 = 41;
pub const MAIN: Range<u16> = 0..36;
pub const HOTBAR: Range<u16> = 0..9;
pub const OFF_HAND: u16 = 40;
pub(crate) const STACK_LIMIT: u8 = 99;
pub(crate) const ARMOUR_LIMIT: u8 = 1;
