use core::fmt;

/// The identity of a world in [`Cells`](crate::Cells), stable for the life of the world.
///
/// [`Cells::add`](crate::Cells::add) gives out the identities in ascending order and never
/// gives one out twice. The worlds tick, and their outputs come back, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellId(pub(crate) u64);

impl fmt::Display for CellId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cell {}", self.0)
    }
}
