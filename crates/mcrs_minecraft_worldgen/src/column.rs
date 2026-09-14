use mcrs_minecraft_core::BlockPos;
use mcrs_voxel_storage::VoxelId;

use crate::feature::placer::WorldGenVolume;

/// `Column`, which is only ever asked for its two edges: `Range` is both
/// present, `Ray` one, `Line` neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    pub floor: Option<i32>,
    pub ceiling: Option<i32>,
}

impl Column {
    pub fn height(&self) -> Option<i32> {
        match (self.floor, self.ceiling) {
            (Some(floor), Some(ceiling)) => Some(ceiling - floor - 1),
            _ => None,
        }
    }
}

/// `Column.scan`: walk out of `pos` in both directions while the column stays
/// inside, and keep an edge only where the block that stopped the walk is one.
pub fn scan_column<W: WorldGenVolume>(
    volume: &W,
    pos: BlockPos,
    search_range: i32,
    inside: impl Fn(VoxelId) -> bool,
    edge: impl Fn(VoxelId) -> bool,
) -> Option<Column> {
    if !inside(volume.get(pos)) {
        return None;
    }
    let scan = |direction: i32| {
        let mut y = pos.y;
        let mut step = 1;
        while step < search_range && inside(volume.get(BlockPos::new(pos.x, y, pos.z))) {
            y += direction;
            step += 1;
        }
        edge(volume.get(BlockPos::new(pos.x, y, pos.z))).then_some(y)
    };
    Some(Column {
        ceiling: scan(1),
        floor: scan(-1),
    })
}
