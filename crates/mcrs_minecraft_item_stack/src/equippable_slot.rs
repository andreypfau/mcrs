#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EquippableSlot {
    Mainhand,
    Feet,
    Legs,
    Chest,
    Head,
    Offhand,
    Body,
    Saddle,
}

impl EquippableSlot {
    pub(crate) const ALL: [Self; 8] = [
        Self::Mainhand,
        Self::Feet,
        Self::Legs,
        Self::Chest,
        Self::Head,
        Self::Offhand,
        Self::Body,
        Self::Saddle,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Mainhand => "mainhand",
            Self::Feet => "feet",
            Self::Legs => "legs",
            Self::Chest => "chest",
            Self::Head => "head",
            Self::Offhand => "offhand",
            Self::Body => "body",
            Self::Saddle => "saddle",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|slot| slot.name() == name)
    }
}
