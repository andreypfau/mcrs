use crate::cell_id::CellId;

/// What the world of `cell` gave for one tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellOutput<O> {
    pub cell: CellId,
    pub output: O,
}
