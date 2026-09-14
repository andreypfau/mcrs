use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::{Blocks, BlocksMut, BoxVolume, Volume, VoxelId};

use crate::feature::placement::HeightmapName;
use crate::value_provider::HeightContext;

use super::{WorldGenVolume, WorldStates};

/// A volume over one owned box: one biome, a fixed extent, a height answered by
/// a closure over the box, and every write logged in order. What a test world
/// needs, and nothing a real column has.
pub struct BoxRegion {
    pub blocks: BoxVolume,
    pub world: WorldStates,
    pub extent: HeightContext,
    pub biome: u32,
    pub height: Box<dyn Fn(&BoxVolume, HeightmapName, i32, i32) -> i32>,
    pub writes: Vec<(BlockPos, VoxelId)>,
}

impl BoxRegion {
    /// Every cell of `min..=max` is `fill`; the extent is the box's own height
    /// and the height map answers one past its top.
    pub fn new(min: BlockPos, max: BlockPos, fill: VoxelId) -> Self {
        BoxRegion {
            blocks: BoxVolume::filled(min, max, fill),
            world: WorldStates::default(),
            extent: HeightContext {
                min_y: min.y,
                depth: max.y - min.y + 1,
                sea_level: 63,
            },
            biome: 0,
            height: Box::new(move |_, _, _, _| max.y + 1),
            writes: Vec::new(),
        }
    }

    /// `radius` columns of sixteen around the origin, from `min_y` to `max_y`.
    pub fn columns(radius: i32, min_y: i32, max_y: i32, fill: VoxelId) -> Self {
        Self::new(
            BlockPos::new(-radius * 16, min_y, -radius * 16),
            BlockPos::new(radius * 16 + 15, max_y, radius * 16 + 15),
            fill,
        )
    }

    /// Every cell from the bottom of the box up to and including `top`.
    pub fn floor(mut self, top: i32, id: VoxelId) -> Self {
        for y in self.blocks.min().y..=top {
            self.blocks.fill_layer(y, id);
        }
        self
    }

    pub fn with_height(
        mut self,
        height: impl Fn(&BoxVolume, HeightmapName, i32, i32) -> i32 + 'static,
    ) -> Self {
        self.height = Box::new(height);
        self
    }
}

impl Volume for BoxRegion {
    fn min(&self) -> BlockPos {
        self.blocks.min()
    }

    fn max(&self) -> BlockPos {
        self.blocks.max()
    }
}

impl Blocks for BoxRegion {
    fn get(&self, p: BlockPos) -> VoxelId {
        self.blocks.get(p)
    }
}

impl BlocksMut for BoxRegion {
    fn set(&mut self, p: BlockPos, id: VoxelId) {
        if self.blocks.contains(p) {
            self.blocks.set(p, id);
            self.writes.push((p, id));
        }
    }
}

impl WorldGenVolume for BoxRegion {
    fn world(&self) -> &WorldStates {
        &self.world
    }

    fn height(&self, kind: HeightmapName, x: i32, z: i32) -> i32 {
        (self.height)(&self.blocks, kind, x, z)
    }

    fn biome(&self, _: BlockPos) -> u32 {
        self.biome
    }

    fn extent(&self) -> HeightContext {
        self.extent
    }

    fn would_survive(&self, _: VoxelId, _: BlockPos) -> bool {
        true
    }
}
