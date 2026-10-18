use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_core::{BlockPos, BoundingBox, SectionPos};

use crate::access::{Access, Volume, section_meets, within};
use crate::bounds::span;
use crate::voxels::{VoxelStore, VoxelStoreMut};

/// An owned dense box of block states: a schematic, a dense test world. Cells of a section
/// it only partly covers read as air and drop writes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BoxVolume {
    bounds: BoundingBox,
    size: [usize; 3],
    cells: Box<[BlockStateId]>,
}

impl BoxVolume {
    /// Every cell of `bounds` holds `state`. `None` when the box holds more cells than
    /// memory can address; a box with `min` above `max` on an axis is empty.
    pub fn filled(bounds: BoundingBox, state: BlockStateId) -> Option<Self> {
        let span = |lo: i32, hi: i32| usize::try_from(span(lo, hi)).ok();
        let size = [
            span(bounds.min.x, bounds.max.x)?,
            span(bounds.min.y, bounds.max.y)?,
            span(bounds.min.z, bounds.max.z)?,
        ];
        let len = size[0].checked_mul(size[1])?.checked_mul(size[2])?;
        let mut cells = Vec::new();
        cells.try_reserve_exact(len).ok()?;
        cells.resize(len, state);
        Some(Self {
            bounds,
            size,
            cells: cells.into_boxed_slice(),
        })
    }

    /// Every cell of one horizontal layer, if the layer is inside the box.
    pub fn fill_layer(&mut self, y: i32, state: BlockStateId) {
        if y < self.bounds.min.y || y > self.bounds.max.y {
            return;
        }
        let dy = offset(y, self.bounds.min.y);
        let [sx, _, sz] = self.size;
        let layer = sx.wrapping_mul(sz);
        let start = dy.wrapping_mul(layer);
        if let Some(cells) = self.cells.get_mut(start..start.wrapping_add(layer)) {
            cells.fill(state);
        }
    }

    /// Every cell with its position, `y`, then `z`, then `x`, `x` fastest.
    pub fn iter(&self) -> impl Iterator<Item = (BlockPos, BlockStateId)> + '_ {
        let (min, max) = (self.bounds.min, self.bounds.max);
        let positions = (min.y..=max.y).flat_map(move |y| {
            (min.z..=max.z).flat_map(move |z| (min.x..=max.x).map(move |x| BlockPos::new(x, y, z)))
        });
        positions.zip(self.cells.iter().copied())
    }

    fn index(&self, at: BlockPos) -> Option<usize> {
        if !within(self.bounds, at) {
            return None;
        }
        let min = self.bounds.min;
        let (x, y, z) = (
            offset(at.x, min.x),
            offset(at.y, min.y),
            offset(at.z, min.z),
        );
        let [sx, _, sz] = self.size;
        Some(
            y.wrapping_mul(sz)
                .wrapping_add(z)
                .wrapping_mul(sx)
                .wrapping_add(x),
        )
    }
}

impl Volume for BoxVolume {
    fn bounds(&self) -> BoundingBox {
        self.bounds
    }

    fn access(&self, section: SectionPos) -> Access {
        if section_meets(self.bounds, section) {
            Access::Write
        } else {
            Access::Outside
        }
    }
}

impl VoxelStore for BoxVolume {
    #[inline]
    fn voxel(&self, at: BlockPos) -> BlockStateId {
        self.index(at)
            .and_then(|index| self.cells.get(index))
            .copied()
            .unwrap_or(BlockStateId::AIR)
    }
}

impl VoxelStoreMut for BoxVolume {
    #[inline]
    fn set_voxel(&mut self, at: BlockPos, state: BlockStateId) -> BlockStateId {
        match self.index(at).and_then(|index| self.cells.get_mut(index)) {
            Some(cell) => std::mem::replace(cell, state),
            None => BlockStateId::AIR,
        }
    }
}

/// `value - min` for a `value` already known to be at least `min`.
fn offset(value: i32, min: i32) -> usize {
    value.abs_diff(min) as usize
}
