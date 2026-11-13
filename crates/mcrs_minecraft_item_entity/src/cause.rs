/// The cause that makes an item entity, a row of the table of IEN-SPN-001.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Cause {
    /// A drop of the hand, a drop of a click or a creative drop.
    Throw,
    /// The rest of the `give` command.
    GiveRest,
    /// The death of a player.
    PlayerDeath,
    /// The cursor stack and the craft grid at a leave, or the rest of the cursor at a menu close.
    MenuRest,
    /// The loot of a mob death or a drop of a mob.
    MobDrop,
    /// A drop of a block.
    BlockDrop,
    /// The contents of a destroyed container block.
    ContainerContents,
    /// The contents of a destroyed item entity.
    ItemContents,
    /// `summon item`.
    Summon,
}

impl Cause {
    /// The pickup delay that the cause starts with.
    #[must_use]
    pub fn pickup_delay(self) -> i16 {
        match self {
            Self::Throw | Self::PlayerDeath | Self::MenuRest => 40,
            Self::MobDrop | Self::BlockDrop => 10,
            Self::GiveRest | Self::ContainerContents | Self::ItemContents | Self::Summon => 0,
        }
    }
}
