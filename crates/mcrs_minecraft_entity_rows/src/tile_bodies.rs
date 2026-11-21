use glam::DVec3;
use mcrs_minecraft_entity_key::{TILE_WIDTH, TileKey};
use mcrs_minecraft_physics::{GridRow, floor_block};

pub(crate) const BUCKET: i32 = 1;
pub(crate) const BUCKETS: usize = (TILE_WIDTH.get() / BUCKET.unsigned_abs()) as usize;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TileBodies {
    pub rows: Vec<GridRow>,
    pub starts: Vec<usize>,
}

pub(crate) fn bucket_xz(tile: TileKey, position: DVec3) -> [usize; 2] {
    let width = TILE_WIDTH.get().cast_signed();
    let local = |value: f64, origin: i32| {
        let block = floor_block(value).saturating_sub(origin.saturating_mul(width));
        usize::try_from(block / BUCKET).map_or(0, |bucket| bucket.min(BUCKETS - 1))
    };
    [local(position.x, tile.x), local(position.z, tile.z)]
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

    pub fn in_box(&self, tile: TileKey, min: DVec3, max: DVec3) -> impl Iterator<Item = &GridRow> {
        let [x0, z0] = bucket_xz(tile, min);
        let [x1, z1] = bucket_xz(tile, max);
        (z0..=z1).flat_map(move |z| {
            let row = z.saturating_mul(BUCKETS);
            self.cells(row.saturating_add(x0), row.saturating_add(x1))
                .iter()
        })
    }
}
