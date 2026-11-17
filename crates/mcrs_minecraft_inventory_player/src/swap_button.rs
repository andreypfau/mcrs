use crate::HOTBAR;
use crate::OFF_HAND;

#[must_use]
pub fn swap_button_slot(button: u8) -> Option<u16> {
    let slot = u16::from(button);
    (HOTBAR.contains(&slot) || slot == OFF_HAND).then_some(slot)
}
