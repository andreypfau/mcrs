/// The order key of a mob or a player: the host packs its birth key into 256 bits in the order of
/// the comparison (tick, source kind, source value, sequence), so the derived order is the order of
/// the birth keys. The key also seeds the keyed random values of the mob.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderKey {
    high: u128,
    low: u128,
}

impl OrderKey {
    #[must_use]
    pub const fn new(high: u128, low: u128) -> Self {
        Self { high, low }
    }

    #[must_use]
    pub const fn high(&self) -> u128 {
        self.high
    }

    #[must_use]
    pub const fn low(&self) -> u128 {
        self.low
    }

    pub(crate) const fn words(&self) -> [u64; 4] {
        [
            (self.high >> 64) as u64,
            self.high as u64,
            (self.low >> 64) as u64,
            self.low as u64,
        ]
    }
}
