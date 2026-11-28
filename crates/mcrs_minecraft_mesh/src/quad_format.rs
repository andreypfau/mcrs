#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
#[repr(u8)]
pub enum QuadFormat {
    #[default]
    Greedy = 0,
    Model = 1,
}

impl QuadFormat {
    pub const COUNT: usize = 2;

    pub const ALL: [Self; Self::COUNT] = [Self::Greedy, Self::Model];

    #[must_use]
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Greedy),
            1 => Some(Self::Model),
            _ => None,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Greedy => "greedy",
            Self::Model => "model",
        }
    }
}
