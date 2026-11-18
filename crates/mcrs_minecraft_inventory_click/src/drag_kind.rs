use crate::DragStage;

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

    #[must_use]
    pub fn button(self, stage: DragStage) -> u8 {
        let kind = match self {
            Self::Split => 0,
            Self::Single => 1,
            Self::Full => 2,
        };
        kind << 2 | stage.number()
    }
}
