use crate::precipitation::PRECIPITATION_TILE;
use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};

const MARGIN: i32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    min_x: i64,
    min_z: i64,
    max_x: i64,
    max_z: i64,
}

impl Region {
    pub(crate) fn of(tile: ColumnPos) -> Self {
        let edge = i64::from(PRECIPITATION_TILE).saturating_mul(SectionPos::SIZE as i64);
        let min = |tile: i32| {
            i64::from(tile)
                .saturating_mul(edge)
                .saturating_sub(i64::from(MARGIN))
        };
        let (min_x, min_z) = (min(tile.x), min(tile.z));
        let last = edge
            .saturating_add(i64::from(MARGIN).saturating_mul(2))
            .saturating_sub(1);
        Self {
            min_x,
            min_z,
            max_x: min_x.saturating_add(last),
            max_z: min_z.saturating_add(last),
        }
    }

    pub(crate) fn holds(&self, section: SectionPos) -> bool {
        let first = |section: i32| i32::try_from(i64::from(section).saturating_mul(16)).ok();
        match (first(section.x), first(section.z)) {
            (Some(x), Some(z)) => self.fits(BlockPos::new(x, 0, z), 0),
            _ => false,
        }
    }

    pub(crate) fn fits(&self, at: BlockPos, reach: u8) -> bool {
        let reach = i64::from(reach);
        let (x, z) = (i64::from(at.x), i64::from(at.z));
        x.saturating_sub(reach) >= self.min_x
            && x.saturating_add(reach) <= self.max_x
            && z.saturating_sub(reach) >= self.min_z
            && z.saturating_add(reach) <= self.max_z
    }
}

pub(crate) fn colour(tile: ColumnPos) -> [i32; 2] {
    [tile.x & 1, tile.z & 1]
}
