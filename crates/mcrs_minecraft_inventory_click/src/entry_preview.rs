#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryPreview {
    Fits(u8),
    Capped(u8),
}

impl EntryPreview {
    #[must_use]
    pub fn count(self) -> u8 {
        match self {
            Self::Fits(count) | Self::Capped(count) => count,
        }
    }
}
