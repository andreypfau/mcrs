#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MobFlags(u8);

impl Default for MobFlags {
    fn default() -> Self {
        Self::PASS_DOORS
    }
}
impl MobFlags {
    pub const OPEN_DOORS: Self = Self(1);
    pub const PASS_DOORS: Self = Self(2);
    pub const FLOAT: Self = Self(4);
    pub const WALK_OVER_FENCES: Self = Self(8);
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
