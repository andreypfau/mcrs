use crate::cell_id::CellId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellOutput<O> {
    pub cell: CellId,
    pub output: O,
}
