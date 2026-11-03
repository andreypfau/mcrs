use rustc_hash::FxHashSet;

use mcrs_minecraft_light_engine::queue::{Priority, PriorityColumns};
use mcrs_minecraft_light_engine::world::Edit;
use mcrs_voxel::ColumnPos;

/// Edits that have not reached the light world yet. The block change itself is never
/// deferred, only the lighting is.
#[derive(Default)]
pub(crate) struct PendingEdits {
    columns: PriorityColumns<Vec<Edit>>,
    /// Columns no budget holds back.
    immediate: FxHashSet<ColumnPos>,
}

impl PendingEdits {
    /// Lower is more urgent. A column keeps the most urgent priority it was
    /// given while it waits.
    pub(crate) fn push_with_priority(&mut self, edit: Edit, priority: Priority) {
        let column = edit.column();
        // A block change takes effect the round it arrives; a section load
        // held back past one in the same column would then overwrite it.
        if matches!(edit, Edit::SetBlock { .. }) {
            self.immediate.insert(column);
        }
        self.columns.entry(column, priority, Vec::new).push(edit);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub(crate) fn reprioritize(&mut self, score: impl FnMut(ColumnPos) -> Priority) {
        self.columns.reprioritize(score);
    }

    /// Whether this column has edits that have not reached the world yet.
    pub(crate) fn holds(&self, column: ColumnPos) -> bool {
        self.columns.contains(column)
    }

    /// The most urgent `limit` columns plus every column holding a block
    /// change, each taken whole.
    ///
    /// Costs what it hands out, never what is waiting: a backlog of thousands
    /// of columns is not re-read to admit fifty.
    pub(crate) fn take_admitted(&mut self, limit: usize) -> Vec<(Priority, Vec<Edit>)> {
        let immediate = std::mem::take(&mut self.immediate);
        let mut admitted: Vec<ColumnPos> = immediate.iter().copied().collect();
        admitted.extend(
            self.columns
                .order()
                .map(|(_, column)| column)
                .filter(|column| !immediate.contains(column))
                .take(limit),
        );
        admitted
            .into_iter()
            .filter_map(|column| self.columns.remove(column))
            .collect()
    }
}
