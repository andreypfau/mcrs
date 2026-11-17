#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LandingEvent {
    BlockBreak,
    AnvilLand,
    AnvilDestroy,
}

impl LandingEvent {
    #[must_use]
    pub fn id(self) -> i32 {
        match self {
            Self::BlockBreak => 2001,
            Self::AnvilLand => 1031,
            Self::AnvilDestroy => 1029,
        }
    }
}
