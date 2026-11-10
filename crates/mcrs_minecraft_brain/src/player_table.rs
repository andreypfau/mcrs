use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::PlayerRow;
use glam::DVec3;

/// The players of one dimension, with an index by 16 × 16 columns for the box queries of a tile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerTable {
    rows: Vec<PlayerRow>,
    columns: BTreeMap<(i32, i32), Vec<u32>>,
}

const COLUMN: f64 = 16.0;

impl PlayerTable {
    /// The table of `rows`, ordered by their order keys.
    #[must_use]
    pub fn new(rows: impl IntoIterator<Item = PlayerRow>) -> Self {
        let mut rows: Vec<PlayerRow> = rows.into_iter().collect();
        rows.sort_by_key(|row| row.key);
        let mut columns: BTreeMap<(i32, i32), Vec<u32>> = BTreeMap::new();
        for (index, row) in rows.iter().enumerate() {
            let Ok(index) = u32::try_from(index) else {
                break;
            };
            columns
                .entry(column(row.position.x, row.position.z))
                .or_default()
                .push(index);
        }
        Self { rows, columns }
    }

    #[must_use]
    pub fn rows(&self) -> &[PlayerRow] {
        &self.rows
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Appends to `out` the players whose position is in the box from `min` to `max` on the `x`
    /// and `z` axes, in the order of their order keys.
    pub fn collect_in(&self, min: DVec3, max: DVec3, out: &mut Vec<PlayerRow>) {
        let start = out.len();
        let (min_x, min_z) = column(min.x, min.z);
        let (max_x, max_z) = column(max.x, max.z);
        for x in min_x..=max_x {
            for (_, indices) in self.columns.range((x, min_z)..=(x, max_z)) {
                for &index in indices {
                    let Some(row) = self.rows.get(index as usize) else {
                        continue;
                    };
                    let inside = (min.x..=max.x).contains(&row.position.x)
                        && (min.z..=max.z).contains(&row.position.z);
                    if inside {
                        out.push(*row);
                    }
                }
            }
        }
        if let Some(found) = out.get_mut(start..) {
            found.sort_by_key(|row| row.key);
        }
    }
}

fn column(x: f64, z: f64) -> (i32, i32) {
    (
        libm::floor(x / COLUMN) as i32,
        libm::floor(z / COLUMN) as i32,
    )
}
