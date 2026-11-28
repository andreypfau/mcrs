use crate::key_modifiers::KeyModifiers;
use crate::pointer_button::PointerButton;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerPress {
    pub x: f64,
    pub y: f64,
    pub button: PointerButton,
    pub modifiers: KeyModifiers,
    pub now_ms: u64,
}
