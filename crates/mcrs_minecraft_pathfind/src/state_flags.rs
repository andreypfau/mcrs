#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateFlags(u8);

impl StateFlags {
    pub const AIR: Self = Self(1);
    pub const LAND_PATHFINDABLE: Self = Self(2);
    pub const FLOATABLE_FLUID: Self = Self(4);
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
