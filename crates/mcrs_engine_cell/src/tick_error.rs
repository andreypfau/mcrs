use core::error::Error;
use core::fmt;

use crate::cell_id::CellId;

/// Why [`Cells::tick`](crate::Cells::tick) sent no tick.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickError {
    /// The world of `cell` is lent out and not given back yet.
    Lent { cell: CellId },
}

impl fmt::Display for TickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lent { cell } => write!(formatter, "the world of {cell} is lent out"),
        }
    }
}

impl Error for TickError {}
