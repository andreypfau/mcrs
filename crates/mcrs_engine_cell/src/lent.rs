use crate::cell_id::CellId;

/// A world lent out of [`Cells`](crate::Cells) between two ticks. Give it back with
/// [`Cells::give_back`](crate::Cells::give_back) before the next tick.
///
/// For tests and tools that read or write a world between ticks. The parts of a server never
/// lend a world: they send messages to it.
#[derive(Debug)]
pub struct Lent<W> {
    pub(crate) cell: CellId,
    pub(crate) world: W,
}

impl<W> Lent<W> {
    pub fn world_mut(&mut self) -> &mut W {
        &mut self.world
    }
}
