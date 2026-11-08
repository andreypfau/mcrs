use crate::CollisionTable;
use crate::SineTable;

/// The tables that the kernel reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    pub collision: CollisionTable,
    pub sine: SineTable,
}

impl Tables {
    #[must_use]
    pub fn new(collision: CollisionTable) -> Self {
        Self {
            collision,
            sine: SineTable::new(),
        }
    }
}
