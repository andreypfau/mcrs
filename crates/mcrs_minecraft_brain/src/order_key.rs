/// The order key of a mob or a player: the host gives the UUID of the entity, so the derived order
/// is the order of the UUIDs. The key also seeds the keyed random values of the mob.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrderKey(u128);

impl OrderKey {
    #[must_use]
    pub const fn new(uuid: u128) -> Self {
        Self(uuid)
    }

    #[must_use]
    pub const fn uuid(&self) -> u128 {
        self.0
    }

    pub(crate) const fn words(&self) -> [u64; 2] {
        [(self.0 >> 64) as u64, self.0 as u64]
    }
}
