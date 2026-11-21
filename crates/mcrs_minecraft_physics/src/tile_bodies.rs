use glam::DVec3;
use mcrs_minecraft_entity_key::EntityUuid;
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
        let mut order: Vec<(usize, EntityUuid, usize)> = bodies
            .iter()
            .enumerate()
            .map(|(index, row)| (bucket(tile, row.position), row.key, index))
            .collect();
        order.sort_unstable();
        self.rows.clear();
        self.rows
            .extend(order.iter().filter_map(|&(.., index)| bodies.get(index)));
        self.starts.clear();
        let mut at = 0;
        for bucket in 0..=BUCKETS * BUCKETS {
            self.starts.push(at);
            while order.get(at).is_some_and(|&(of, ..)| of == bucket) {
                at += 1;
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
