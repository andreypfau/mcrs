use crate::key_modifiers::KeyModifiers;
use crate::pointer_button::PointerButton;

/// A press of `button` at the event position (`x`, `y`) at `now_ms` (WID-PTR-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerPress {
    pub x: f64,
    pub y: f64,
    pub button: PointerButton,
    pub modifiers: KeyModifiers,
    pub now_ms: u64,
}
