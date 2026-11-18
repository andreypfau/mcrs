#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragStage {
    Start,
    Entry,
    End,
}

impl DragStage {
    pub(crate) fn number(self) -> u8 {
        match self {
            Self::Start => 0,
            Self::Entry => 1,
            Self::End => 2,
        }
    }

    pub(crate) fn from_button(button: u8) -> Option<Self> {
        match button & 3 {
            0 => Some(Self::Start),
            1 => Some(Self::Entry),
            2 => Some(Self::End),
            _ => None,
        }
    }
}
