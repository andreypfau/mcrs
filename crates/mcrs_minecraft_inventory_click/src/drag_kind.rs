#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragKind {
    Split,
    Single,
    Full,
}

impl DragKind {
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
