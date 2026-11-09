use crate::Crossing;
use crate::Placed;

/// The living rows that came into the world, changed tile or left it. Each list keeps the
/// changes of its last phase: `spawned` the spawns since the start of the last pass, `crossed`
/// the last tile moves, and `removed` the deaths of the last pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    pub(crate) spawned: Vec<Placed>,
    pub(crate) crossed: Vec<Crossing>,
    pub(crate) removed: Vec<Placed>,
}

impl Changes {
    /// The living rows that a spawn added since the start of the last pass, in spawn order.
    #[must_use]
    pub fn spawned(&self) -> &[Placed] {
        &self.spawned
    }

    /// The living rows that the last tile moves took to another tile.
    #[must_use]
    pub fn crossed(&self) -> &[Crossing] {
        &self.crossed
    }

    /// The living rows that left the world in the last pass at the end of their death.
    #[must_use]
    pub fn removed(&self) -> &[Placed] {
        &self.removed
    }
}
