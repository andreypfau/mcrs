use crate::cell_id::CellId;

/// A world lent out of [`Cells`](crate::Cells) between two ticks. Give it back with
/// [`Cells::give_back`](crate::Cells::give_back) before the next tick.
///
/// Transitional: a pass that reads or writes several worlds at once borrows them this way,
/// until it becomes messages between the worlds.
#[derive(Debug)]
pub struct Lent<W> {
    pub(crate) cell: CellId,
    pub(crate) world: W,
}

impl<W> Lent<W> {
    #[must_use]
    pub fn cell(&self) -> CellId {
        self.cell
    }

    #[must_use]
    pub fn world(&self) -> &W {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut W {
        &mut self.world
    }
}
