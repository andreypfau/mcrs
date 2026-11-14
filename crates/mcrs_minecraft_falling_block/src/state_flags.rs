#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct StateFlags(u16);

impl StateFlags {
    pub(crate) const FREE: Self = Self(1 << 0);
    pub(crate) const GRAVITY: Self = Self(1 << 1);
    pub(crate) const BREAKS_AFTER_FALL: Self = Self(1 << 2);
    pub(crate) const HOLDS_WATER: Self = Self(1 << 3);
    pub(crate) const WATER_SOURCE: Self = Self(1 << 4);
    pub(crate) const MOVING_PISTON: Self = Self(1 << 5);
    pub(crate) const ANVIL: Self = Self(1 << 6);
    pub(crate) const BREAK_PARTICLES: Self = Self(1 << 7);

    pub(crate) fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    pub(crate) fn set(&mut self, flag: Self, present: bool) {
        if present {
            self.0 |= flag.0;
        }
    }
}
