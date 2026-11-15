use crate::HOTBAR;
use crate::OFF_HAND;

/// The slot of the player inventory that the button of a swap click names, in each menu.
/// Returns `None` for a button that names no slot.
#[must_use]
pub fn swap_button_slot(button: u8) -> Option<u16> {
    let slot = u16::from(button);
    (HOTBAR.contains(&slot) || slot == OFF_HAND).then_some(slot)
}
