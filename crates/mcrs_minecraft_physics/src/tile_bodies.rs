use glam::DVec3;
use mcrs_minecraft_entity_key::TILE_WIDTH;
use mcrs_minecraft_entity_key::TileKey;

use crate::GridRow;
use crate::floor_block;

const BUCKET: i32 = 1;
pub(crate) const BUCKETS: usize = (TILE_WIDTH.get() / BUCKET.unsigned_abs()) as usize;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileBodies {
    rows: Vec<GridRow>,
    starts: Vec<usize>,
}

pub(crate) static NO_BODIES: TileBodies = TileBodies {
    rows: Vec::new(),
    starts: Vec::new(),
};

impl TileBodies {
    pub fn fill(&mut self, tile: TileKey, bodies: &[GridRow]) -> f32 {
        self.starts.clear();
        self.starts.resize(BUCKETS * BUCKETS + 1, 0);
        for body in bodies {
            if let Some(count) = self.starts.get_mut(bucket(tile, body.position)) {
                *count += 1;
            }
        }
        let mut total = 0;
        for start in &mut self.starts {
            let count = *start;
            *start = total;
            total += count;
        }
        self.rows.clear();
        self.rows.extend_from_slice(bodies);
        for body in bodies {
            let Some(cursor) = self.starts.get_mut(bucket(tile, body.position)) else {
                continue;
            };
            if let Some(slot) = self.rows.get_mut(*cursor) {
                *slot = *body;
            }
            *cursor += 1;
        }
        self.starts.rotate_right(1);
        if let Some(first) = self.starts.first_mut() {
            *first = 0;
        }
        for cell in self.starts.windows(2) {
            if let [start, end] = *cell
                && end > start.saturating_add(1)
                && let Some(rows) = self.rows.get_mut(start..end)
            {
                rows.sort_unstable_by_key(|row| row.key);
            }
        }
        bodies
            .iter()
            .map(|body| body.half_width)
            .fold(0.0, f32::max)
    }

    #[must_use]
    pub fn rows(&self) -> &[GridRow] {
        &self.rows
    }

    pub fn in_box(&self, tile: TileKey, min: DVec3, max: DVec3) -> impl Iterator<Item = &GridRow> {
        let [x0, z0] = bucket_xz(tile, min);
        let [x1, z1] = bucket_xz(tile, max);
        (z0..=z1).flat_map(move |z| {
            let row = z.saturating_mul(BUCKETS);
            self.cells(row.saturating_add(x0), row.saturating_add(x1))
                .iter()
        })
    }

    #[must_use]
    pub(crate) fn cells(&self, first: usize, last: usize) -> &[GridRow] {
        let start = self.starts.get(first).copied().unwrap_or_default();
        let end = last
            .checked_add(1)
            .and_then(|next| self.starts.get(next))
            .copied()
            .unwrap_or_default();
        self.rows.get(start..end).unwrap_or_default()
    }
}

pub(crate) fn bucket_xz(tile: TileKey, position: DVec3) -> [usize; 2] {
    let width = TILE_WIDTH.get().cast_signed();
    let local = |value: f64, origin: i32| {
        let block = floor_block(value).saturating_sub(origin.saturating_mul(width));
        usize::try_from(block / BUCKET).map_or(0, |bucket| bucket.min(BUCKETS - 1))
    };
    [local(position.x, tile.x), local(position.z, tile.z)]
}

fn bucket(tile: TileKey, position: DVec3) -> usize {
    let [x, z] = bucket_xz(tile, position);
    z * BUCKETS + x
}
