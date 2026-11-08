use crate::GridRow;

/// The bodies of one tile of the grid, grouped by the cells that the writer of the grid picks:
/// the rows of cell `c` are `rows[starts[c]..starts[c + 1]]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileBodies {
    pub rows: Vec<GridRow>,
    pub starts: Vec<usize>,
}

impl TileBodies {
    /// The rows of the cells from `first` to `last`; none when a cell is not in the tile.
    #[must_use]
    pub fn cells(&self, first: usize, last: usize) -> &[GridRow] {
        let start = self.starts.get(first).copied().unwrap_or_default();
        let end = last
            .checked_add(1)
            .and_then(|next| self.starts.get(next))
            .copied()
            .unwrap_or_default();
        self.rows.get(start..end).unwrap_or_default()
    }
}
