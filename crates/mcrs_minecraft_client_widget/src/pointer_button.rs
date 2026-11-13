/// A mouse button. Each widget of this crate accepts only the left button (WID-PTR-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}
