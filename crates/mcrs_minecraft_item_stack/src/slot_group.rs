/// The equipment slots in which an attribute modifier of a stack applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SlotGroup {
    #[default]
    Any,
    Mainhand,
    Offhand,
    Hand,
    Feet,
    Legs,
    Chest,
    Head,
    Armor,
    Body,
    Saddle,
}

impl SlotGroup {
    /// Each variant, in the order of the wire ids.
    pub(crate) const ALL: [Self; 11] = [
        Self::Any,
        Self::Mainhand,
        Self::Offhand,
        Self::Hand,
        Self::Feet,
        Self::Legs,
        Self::Chest,
        Self::Head,
        Self::Armor,
        Self::Body,
        Self::Saddle,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Mainhand => "mainhand",
            Self::Offhand => "offhand",
            Self::Hand => "hand",
            Self::Feet => "feet",
            Self::Legs => "legs",
            Self::Chest => "chest",
            Self::Head => "head",
            Self::Armor => "armor",
            Self::Body => "body",
            Self::Saddle => "saddle",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|group| group.name() == name)
    }
}
