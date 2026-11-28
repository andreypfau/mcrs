#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ControlFlags(u8);

impl ControlFlags {
    pub const NONE: Self = Self(0);
    pub const MOVE: Self = Self(1);
    pub const LOOK: Self = Self(2);
    pub const JUMP: Self = Self(4);
    pub const TARGET: Self = Self(8);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
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
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn slots(self) -> impl Iterator<Item = usize> {
        (0..4).filter(move |slot| self.0 & (1 << slot) != 0)
    }
}
