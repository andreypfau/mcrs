use mcrs_minecraft_physics::GridRow;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileBodies {
    pub rows: Vec<GridRow>,
    pub starts: Vec<usize>,
}

impl TileBodies {
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
