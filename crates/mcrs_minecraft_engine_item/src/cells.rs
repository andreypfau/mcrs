use bevy_ecs::component::Component;
use bevy_ecs::storage::Table;
use bevy_ecs::storage::TableRow;
use mcrs_minecraft_item_stack::ItemComponent;

use crate::Removed;
use crate::column::Column;
use crate::effective;

/// The table row of one stack and one bit for each row of the item components that it stores.
#[derive(Clone, Copy)]
pub(crate) struct Cells<'w> {
    table: &'w Table,
    row: TableRow,
    mask: u128,
    removed: Option<&'w Removed>,
}

impl<'w> Cells<'w> {
    /// # Safety
    /// `row` is the row of a live entity in `table` for `'w`, and the caller may read the column
    /// of each row of the item components and of [`Removed`] for `'w`.
    pub(crate) unsafe fn new(
        table: &'w Table,
        row: TableRow,
        mask: u128,
        removed: Column<Removed>,
    ) -> Self {
        let mut cells = Self {
            table,
            row,
            mask,
            removed: None,
        };
        cells.removed = cells.get(removed);
        cells
    }

    pub(crate) fn mask(&self) -> u128 {
        self.mask
    }

    pub(crate) fn removed(&self) -> Option<&'w Removed> {
        self.removed
    }

    /// The rows whose effective value can differ from the neutral value.
    pub(crate) fn present(&self) -> u128 {
        self.mask | self.removed.map_or(0, |removed| u128::from(removed.bits()))
    }

    /// The stored value of `T`. The column is that of an item component row or of [`Removed`].
    pub(crate) fn get<T: Component>(&self, column: Column<T>) -> Option<&'w T> {
        let data = self.table.get_column(column.id())?;
        // SAFETY: by the contract of `new`, `row` is a live row of `table` that the holder may
        // read for `'w`; the column holds values of `T`, by the contract of `Column`.
        Some(unsafe { data.get_data_unchecked(self.row).deref::<T>() })
    }

    pub(crate) fn effective<T: ItemComponent + Component>(
        &self,
        column: Column<T>,
    ) -> Option<&'w T> {
        effective(self.get(column), self.removed)
    }
}
