#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AiDelay {
    AtLeast(u8),
    Fixed(u8),
}

impl AiDelay {
    #[must_use]
    pub const fn ticks(&self) -> u8 {
        match self {
            Self::AtLeast(ticks) | Self::Fixed(ticks) => *ticks,
        }
    }
}
