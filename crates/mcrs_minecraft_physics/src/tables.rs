use crate::CollisionTable;

#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    pub collision: CollisionTable,
}

impl Tables {
    #[must_use]
    pub fn new(collision: CollisionTable) -> Self {
        Self { collision }
    }
}
