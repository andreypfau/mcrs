use crate::InsideEffect;

/// The stuck multipliers that an entity type ignores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StuckIgnore {
    None,
    One(InsideEffect),
    Every,
}

impl StuckIgnore {
    #[must_use]
    pub fn ignores(self, effect: InsideEffect) -> bool {
        match self {
            StuckIgnore::None => false,
            StuckIgnore::One(ignored) => ignored == effect,
            StuckIgnore::Every => true,
        }
    }
}
