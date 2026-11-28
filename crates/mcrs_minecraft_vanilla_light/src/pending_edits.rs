use rustc_hash::FxHashSet;

use mcrs_minecraft_light_engine::queue::{Priority, PriorityColumns};
use mcrs_minecraft_light_engine::world::Edit;
use mcrs_voxel::ColumnPos;

#[derive(Default)]
pub(crate) struct PendingEdits {
    columns: PriorityColumns<Vec<Edit>>,
    immediate: FxHashSet<ColumnPos>,
}

impl PendingEdits {
    pub(crate) fn push_with_priority(&mut self, edit: Edit, priority: Priority) {
        let column = edit.column();
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

    pub(crate) fn holds(&self, column: ColumnPos) -> bool {
        self.columns.contains(column)
    }

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
