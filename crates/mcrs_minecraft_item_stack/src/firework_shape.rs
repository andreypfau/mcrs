#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FireworkShape {
    SmallBall,
    LargeBall,
    Star,
    Creeper,
    Burst,
}

impl FireworkShape {
    pub(crate) const ALL: [Self; 5] = [
        Self::SmallBall,
        Self::LargeBall,
        Self::Star,
        Self::Creeper,
        Self::Burst,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::SmallBall => "small_ball",
            Self::LargeBall => "large_ball",
            Self::Star => "star",
            Self::Creeper => "creeper",
            Self::Burst => "burst",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|shape| shape.name() == name)
    }
}
