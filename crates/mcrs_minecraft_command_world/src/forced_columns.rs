use std::collections::BTreeSet;

use bevy_ecs::resource::Resource;
use mcrs_voxel::ColumnPos;

/// The forced columns of the dimension of this world. A forced column holds a loading ticket,
/// so it stays loaded and its entities tick with no player near.
#[derive(Resource, Debug, Default)]
pub struct ForcedColumns(BTreeSet<i64>);

/// The order of the forced columns: the signed key `(cz << 32) | (cx & 0xFFFFFFFF)`.
fn key(column: ColumnPos) -> i64 {
    (i64::from(column.z) << 32) | (i64::from(column.x) & 0xFFFF_FFFF)
}

fn column_of(key: i64) -> ColumnPos {
    ColumnPos::new(key as i32, (key >> 32) as i32)
}

impl ForcedColumns {
    /// Marks `column`; `false` when it was marked.
    pub fn insert(&mut self, column: ColumnPos) -> bool {
        self.0.insert(key(column))
    }

    /// Clears the mark of `column`; `false` when it had none.
    pub fn remove(&mut self, column: ColumnPos) -> bool {
        self.0.remove(&key(column))
    }

    #[must_use]
    pub fn contains(&self, column: ColumnPos) -> bool {
        self.0.contains(&key(column))
    }

    /// The forced columns in their order.
    pub fn iter(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.0.iter().map(|&key| column_of(key))
    }

    /// Clears every mark, and gives back the columns that had one.
    pub fn take(&mut self) -> impl Iterator<Item = ColumnPos> + use<> {
        std::mem::take(&mut self.0).into_iter().map(column_of)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_columns_list_by_z_then_by_the_unsigned_x() {
        let mut forced = ForcedColumns::default();
        for (x, z) in [(0, 0), (-1, 0), (5, -1), (0, 1)] {
            forced.insert(ColumnPos::new(x, z));
        }
        let listed: Vec<(i32, i32)> = forced.iter().map(|column| (column.x, column.z)).collect();
        assert_eq!(listed, [(5, -1), (0, 0), (-1, 0), (0, 1)]);
    }
}
