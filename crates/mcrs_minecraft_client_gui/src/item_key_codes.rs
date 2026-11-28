use bevy::prelude::KeyCode;
use mcrs_minecraft_container_click::Gesture;

pub const DROP: KeyCode = KeyCode::KeyQ;
pub const SWAP_OFFHAND: KeyCode = KeyCode::KeyF;
pub const SHIFT: [KeyCode; 2] = [KeyCode::ShiftLeft, KeyCode::ShiftRight];
pub const CONTROL: [KeyCode; 2] = [KeyCode::ControlLeft, KeyCode::ControlRight];
pub const HOTBAR: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

pub(crate) fn key_gesture(key: KeyCode) -> Option<Gesture> {
    match key {
        SWAP_OFFHAND => Some(Gesture::OffhandKey),
        DROP => Some(Gesture::DropKey),
        _ => {
            let index = HOTBAR.iter().position(|&hotbar| hotbar == key)?;
            let number = u8::try_from(index).ok()?.checked_add(1)?;
            Some(Gesture::HotbarKey(number))
        }
    }
}
