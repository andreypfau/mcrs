/// An armour slot of the player inventory, named as the `slot` of the `minecraft:equippable`
/// component names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ArmourSlot {
    Feet,
    Legs,
    Chest,
    Head,
}

impl ArmourSlot {
    /// The index of this slot in the player inventory.
    #[must_use]
    pub const fn index(self) -> u16 {
        match self {
            Self::Feet => 36,
            Self::Legs => 37,
            Self::Chest => 38,
            Self::Head => 39,
        }
    }

    /// The armour slot at `index` of the player inventory.
    #[must_use]
    pub const fn from_index(index: u16) -> Option<Self> {
        match index {
            36 => Some(Self::Feet),
            37 => Some(Self::Legs),
            38 => Some(Self::Chest),
            39 => Some(Self::Head),
            _ => None,
        }
    }
}
