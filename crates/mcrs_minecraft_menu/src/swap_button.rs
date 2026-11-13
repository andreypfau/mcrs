/// The slot of the player inventory that the button of a swap click names, in each menu.
/// Returns `None` for a button that names no slot.
#[must_use]
pub fn swap_button_slot(button: u8) -> Option<u16> {
    match button {
        0..=8 | 40 => Some(u16::from(button)),
        _ => None,
    }
}
