/// Whether the components of one type take part in the row hash of a stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowHashing {
    Included,
    Excluded,
}
