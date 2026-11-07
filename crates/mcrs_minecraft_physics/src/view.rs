use crate::Tables;

/// The read-only data of every step: the blocks, the other entities and the tables.
pub struct View<'a, W, B> {
    pub world: &'a W,
    pub bodies: &'a B,
    pub tables: &'a Tables,
}

impl<W, B> Clone for View<'_, W, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W, B> Copy for View<'_, W, B> {}

impl<'a, W, B> View<'a, W, B> {
    #[must_use]
    pub fn new(world: &'a W, bodies: &'a B, tables: &'a Tables) -> Self {
        Self {
            world,
            bodies,
            tables,
        }
    }
}
