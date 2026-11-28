use crate::cell_id::CellId;

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
