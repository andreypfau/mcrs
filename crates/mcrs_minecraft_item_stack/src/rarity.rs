/// The rarity of the stack, which sets the colour of its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl Rarity {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
            Self::Epic => "epic",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        [Self::Common, Self::Uncommon, Self::Rare, Self::Epic]
            .into_iter()
            .find(|rarity| rarity.name() == name)
    }
}
