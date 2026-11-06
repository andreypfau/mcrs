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
