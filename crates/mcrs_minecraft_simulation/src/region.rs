use crate::precipitation::PRECIPITATION_TILE;
use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};

const MARGIN: i32 = 32;
pub(crate) const CELL: i32 = 32;

#[must_use]
pub(crate) fn cell_key(pos: BlockPos) -> u64 {
    let x = pos.x.div_euclid(CELL);
    let z = pos.z.div_euclid(CELL);
    (u64::from(x.cast_unsigned()) << 32) | u64::from(z.cast_unsigned())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    min_x: i64,
    min_z: i64,
    max_x: i64,
    max_z: i64,
}

impl Region {
    pub(crate) const WHOLE: Self = Self {
        min_x: i64::MIN,
        min_z: i64::MIN,
        max_x: i64::MAX,
        max_z: i64::MAX,
    };

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

    pub(crate) fn cells(&self) -> impl Iterator<Item = u64> {
        let first_x = self.min_x.div_euclid(i64::from(CELL));
        let last_x = self.max_x.div_euclid(i64::from(CELL));
        let first_z = self.min_z.div_euclid(i64::from(CELL));
        let last_z = self.max_z.div_euclid(i64::from(CELL));
        (first_x..=last_x).flat_map(move |x| {
            (first_z..=last_z).filter_map(move |z| {
                let x = i32::try_from(x).ok()?;
                let z = i32::try_from(z).ok()?;
                Some(cell_key(BlockPos::new(
                    x.saturating_mul(CELL),
                    0,
                    z.saturating_mul(CELL),
                )))
            })
        })
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Touch {
    min: BlockPos,
    max: BlockPos,
}

impl Touch {
    pub(crate) const EMPTY: Self = Self {
        min: BlockPos::new(i32::MAX, i32::MAX, i32::MAX),
        max: BlockPos::new(i32::MIN, i32::MIN, i32::MIN),
    };

    pub(crate) fn extend(&mut self, at: BlockPos) {
        self.min = BlockPos::new(
            self.min.x.min(at.x),
            self.min.y.min(at.y),
            self.min.z.min(at.z),
        );
        self.max = BlockPos::new(
            self.max.x.max(at.x),
            self.max.y.max(at.y),
            self.max.z.max(at.z),
        );
    }

    pub(crate) fn extend_section(&mut self, section: SectionPos) {
        let first = |axis: i32| axis.saturating_mul(SectionPos::SIZE as i32);
        let last =
            |axis: i32| first(axis).saturating_add((SectionPos::SIZE as i32).saturating_sub(1));
        self.extend(BlockPos::new(
            first(section.x),
            first(section.y),
            first(section.z),
        ));
        self.extend(BlockPos::new(
            last(section.x),
            last(section.y),
            last(section.z),
        ));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    pub(crate) fn intersects(&self, other: &Self) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
            && self.min.z <= other.max.z
            && other.min.z <= self.max.z
    }

    pub(crate) fn cells(&self) -> impl Iterator<Item = u64> {
        let (first_x, last_x, first_z, last_z) = if self.is_empty() {
            (0, -1, 0, -1)
        } else {
            (
                self.min.x.div_euclid(CELL),
                self.max.x.div_euclid(CELL),
                self.min.z.div_euclid(CELL),
                self.max.z.div_euclid(CELL),
            )
        };
        (first_x..=last_x).flat_map(move |x| {
            (first_z..=last_z).map(move |z| {
                cell_key(BlockPos::new(
                    x.saturating_mul(CELL),
                    0,
                    z.saturating_mul(CELL),
                ))
            })
        })
    }
}
