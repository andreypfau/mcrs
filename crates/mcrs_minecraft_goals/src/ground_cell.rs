/// What the goals of a mob type read of one block position: the floor, the solid flag, water,
/// the malus of its path type for the mob type, and the walk value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroundCell {
    pub flags: CellFlags,
    /// The malus of the mob path type of the position for the mob type; 0 is free, -1 is blocked.
    pub malus: f32,
    /// The walk value of the position for the mob type (GOAL-POS-004).
    pub walk_value: f32,
}

impl GroundCell {
    /// A stable floor with no solid block, at no malus: a mob can stand here.
    #[must_use]
    pub fn is_walkable(&self) -> bool {
        self.flags.contains(CellFlags::STABLE_FLOOR)
            && !self.flags.contains(CellFlags::LEGACY_SOLID)
            && self.malus == 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CellFlags(u8);

impl CellFlags {
    pub const NONE: Self = Self(0);
    /// The block below has a solid render state.
    pub const STABLE_FLOOR: Self = Self(1);
    /// The block has the legacy solid flag.
    pub const LEGACY_SOLID: Self = Self(1 << 1);
    /// The block has water.
    pub const WATER: Self = Self(1 << 2);
    /// The collision shape of the block is empty.
    pub const NO_COLLISION: Self = Self(1 << 3);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// The lowest and the highest `y` of the build height of a dimension, both inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BuildHeight {
    pub min: i32,
    pub max: i32,
}

impl BuildHeight {
    #[must_use]
    pub const fn contains(&self, y: i32) -> bool {
        self.min <= y && y <= self.max
    }
}
