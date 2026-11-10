use std::collections::BTreeSet;

use bevy_ecs::resource::Resource;
use mcrs_voxel::ColumnPos;

/// The forced columns of the dimension of this world. A forced column holds a loading ticket,
/// so it stays loaded and its entities tick with no player near.
#[derive(Resource, Debug, Default)]
pub struct ForcedColumns(BTreeSet<ForcedKey>);

/// A forced column in the order of the list: by z, then by the bits of x read as unsigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ForcedKey {
    z: i32,
    x: u32,
}

impl From<ColumnPos> for ForcedKey {
    fn from(column: ColumnPos) -> Self {
        Self {
            z: column.z,
            x: column.x.cast_unsigned(),
        }
    }
}

impl From<ForcedKey> for ColumnPos {
    fn from(key: ForcedKey) -> Self {
        ColumnPos::new(key.x.cast_signed(), key.z)
    }
}

impl ForcedColumns {
    /// Marks `column`; `false` when it was marked.
    pub fn insert(&mut self, column: ColumnPos) -> bool {
        self.0.insert(column.into())
    }

    /// Clears the mark of `column`; `false` when it had none.
    pub fn remove(&mut self, column: ColumnPos) -> bool {
        self.0.remove(&column.into())
    }

    #[must_use]
    pub fn contains(&self, column: ColumnPos) -> bool {
        self.0.contains(&column.into())
    }

    /// The forced columns in their order.
    pub fn iter(&self) -> impl Iterator<Item = ColumnPos> + '_ {
        self.0.iter().map(|&key| key.into())
    }

    /// Clears every mark, and gives back the columns that had one.
    pub fn take(&mut self) -> impl Iterator<Item = ColumnPos> + use<> {
        std::mem::take(&mut self.0).into_iter().map(ColumnPos::from)
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
