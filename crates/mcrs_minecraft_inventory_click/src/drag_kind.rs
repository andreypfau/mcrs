/// What a drag gives to each slot of its set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragKind {
    /// An equal part of the cursor stack.
    Split,
    /// One item.
    Single,
    /// A full stack that the drag makes; only for a player with infinite materials.
    Full,
}

impl DragKind {
    /// The drag kind in the bits 2 and 3 of a quick craft button, or `None` for the value 3.
    #[must_use]
    pub fn from_button(button: u8) -> Option<Self> {
        match (button >> 2) & 3 {
            0 => Some(Self::Split),
            1 => Some(Self::Single),
            2 => Some(Self::Full),
            _ => None,
        }
    }
}
