/// The path types of PATH-TYP-001 that the cells of a mob of width 1 can have, in the order of
/// their numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PathType {
    Blocked,
    #[default]
    Open,
    Walkable,
    WalkableDoor,
    DropDown,
    PowderSnow,
    OnTopOfPowderSnow,
    Fence,
    Lava,
    Water,
    WaterBorder,
    Rail,
    UnpassableRail,
    FireInNeighbor,
    Fire,
    DamagingInNeighbor,
    Damaging,
    DoorOpen,
    DoorWoodClosed,
    DoorIronClosed,
    Breach,
    Leaves,
    Sticky,
    AvoidInAir,
    DamageCautious,
    OnTopOfDropDown,
    BigMobsCloseToDanger,
}

impl PathType {
    /// The default malus of PATH-TYP-001; -1 is blocked.
    #[must_use]
    pub const fn default_malus(self) -> f32 {
        match self {
            Self::Blocked
            | Self::PowderSnow
            | Self::Fence
            | Self::Lava
            | Self::UnpassableRail
            | Self::Damaging
            | Self::DoorWoodClosed
            | Self::DoorIronClosed
            | Self::Leaves => -1.0,
            Self::Open
            | Self::Walkable
            | Self::WalkableDoor
            | Self::DropDown
            | Self::OnTopOfPowderSnow
            | Self::Rail
            | Self::DoorOpen
            | Self::AvoidInAir
            | Self::DamageCautious
            | Self::OnTopOfDropDown => 0.0,
            Self::Breach | Self::BigMobsCloseToDanger => 4.0,
            Self::Water
            | Self::WaterBorder
            | Self::FireInNeighbor
            | Self::DamagingInNeighbor
            | Self::Sticky => 8.0,
            Self::Fire => 16.0,
        }
    }

    /// The block path type at a position whose state path type is open, from the state path type
    /// of the block below it (PATH-TYP-003, step 2); `None` asks for the neighbour check.
    #[must_use]
    pub const fn over(below: Self) -> Option<Self> {
        match below {
            Self::Open | Self::Water | Self::Lava | Self::Walkable => Some(Self::Open),
            Self::Fire => Some(Self::Fire),
            Self::Damaging => Some(Self::Damaging),
            Self::Sticky => Some(Self::Sticky),
            Self::PowderSnow => Some(Self::OnTopOfPowderSnow),
            Self::DamageCautious => Some(Self::DamageCautious),
            Self::DropDown => Some(Self::OnTopOfDropDown),
            _ => None,
        }
    }

    /// The result of the neighbour check for a neighbour of this state path type (PATH-TYP-003,
    /// step 3); `None` when the neighbour does not decide it.
    #[must_use]
    pub const fn as_neighbour(self) -> Option<Self> {
        match self {
            Self::Damaging => Some(Self::DamagingInNeighbor),
            Self::Fire | Self::Lava => Some(Self::FireInNeighbor),
            Self::Water => Some(Self::WaterBorder),
            Self::DamageCautious => Some(Self::DamageCautious),
            _ => None,
        }
    }
}
