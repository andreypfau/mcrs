#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Cause {
    Throw,
    GiveRest,
    PlayerDeath,
    MenuRest,
    MobDrop,
    BlockDrop,
    ContainerContents,
    ItemContents,
    Summon,
}

impl Cause {
    #[must_use]
    pub fn pickup_delay(self) -> i16 {
        match self {
            Self::Throw | Self::PlayerDeath | Self::MenuRest => 40,
            Self::MobDrop | Self::BlockDrop => 10,
            Self::GiveRest | Self::ContainerContents | Self::ItemContents | Self::Summon => 0,
        }
    }
}
