#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DamageTags(u16);

impl DamageTags {
    pub const BYPASSES_ARMOR: Self = Self(1 << 0);
    pub const BYPASSES_COOLDOWN: Self = Self(1 << 1);
    pub const BYPASSES_EFFECTS: Self = Self(1 << 2);
    pub const BYPASSES_RESISTANCE: Self = Self(1 << 3);
    pub const IS_FIRE: Self = Self(1 << 4);
    pub const IS_FALL: Self = Self(1 << 5);
    pub const NO_KNOCKBACK: Self = Self(1 << 6);
    pub const BYPASSES_INVULNERABILITY: Self = Self(1 << 7);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
