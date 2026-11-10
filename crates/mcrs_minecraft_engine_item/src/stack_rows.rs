use core::hash::Hasher;

use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_item_stack::Kind;
use rustc_hash::FxHasher;

use crate::cells::Cells;
use crate::item_columns::ItemColumns;
use crate::row::Row;
use crate::row::bits;

/// Compares and hashes the item components of stacks. It reads every item component type and
/// [`Kind`], not the count.
///
/// A system that also writes an item component takes both through a `ParamSet`.
#[derive(SystemParam)]
pub struct StackRows<'w, 's> {
    columns: ItemColumns<'w, 's>,
    kinds: Query<'w, 's, &'static Kind>,
}

impl<'w, 's> StackRows<'w, 's> {
    #[must_use]
    pub fn kind(&self, stack: Entity) -> Option<Kind> {
        self.kinds.get(stack).ok().copied()
    }

    /// True when the two stacks have the same kind and the same effective components. The count
    /// has no part.
    #[must_use]
    pub fn mergeable(&mut self, left: Entity, right: Entity) -> bool {
        if self.kind(left).is_none() || self.kind(left) != self.kind(right) {
            return false;
        }
        match (self.columns.cells(left), self.columns.cells(right)) {
            (Some(left), Some(right)) => equal(self.columns.rows(), &left, &right),
            _ => false,
        }
    }

    /// A hash of the kind and the effective components. Mergeable stacks have equal hashes.
    #[must_use]
    pub fn row_hash(&mut self, stack: Entity) -> Option<u64> {
        let kind = self.kind(stack)?;
        let cells = self.columns.cells(stack)?;
        Some(row_hash(self.columns.rows(), kind, &cells))
    }

    pub(crate) fn columns(&mut self) -> &mut ItemColumns<'w, 's> {
        &mut self.columns
    }
}

fn equal(rows: &[Row], left: &Cells, right: &Cells) -> bool {
    bits(left.present() | right.present())
        .all(|index| rows.get(index).is_some_and(|row| row.equal(left, right)))
}

fn row_hash(rows: &[Row], kind: Kind, cells: &Cells) -> u64 {
    let mut hasher = FxHasher::default();
    hasher.write_u16(kind.0);
    for index in bits(cells.present()) {
        if let Some(row) = rows.get(index) {
            row.hash(cells, index, &mut hasher);
        }
    }
    hasher.finish()
}
