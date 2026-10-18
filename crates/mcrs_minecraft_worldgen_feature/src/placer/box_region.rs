use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_core::{BlockPos, BoundingBox, SectionPos};
use mcrs_minecraft_volume::{Access, BoxVolume, Volume, VoxelStore, VoxelStoreMut};

use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use mcrs_minecraft_value_provider::HeightContext;

use super::{StateCount, WorldGenVolume, WorldStates};

fn test_state(voxel: VoxelId) -> BlockStateId {
    BlockStateId::checked(voxel.0, StateCount::default().0).expect("a test world has no table")
}

type HeightFn = Box<dyn Fn(&BoxVolume, HeightmapName, i32, i32) -> i32>;

/// A volume over one owned box: one biome, a fixed extent, a height answered by
/// a closure over the box, and every write logged in order. What a test world
/// needs, and nothing a real column has.
pub struct BoxRegion {
    pub blocks: BoxVolume,
    pub world: WorldStates,
    pub extent: HeightContext,
    pub biome: u16,
    pub height: HeightFn,
    pub writes: Vec<(BlockPos, VoxelId)>,
}

impl BoxRegion {
    /// Every cell of `min..=max` is `fill`; the extent is the box's own height
    /// and the height map answers one past its top.
    pub fn new(min: BlockPos, max: BlockPos, fill: VoxelId) -> Self {
        BoxRegion {
            blocks: BoxVolume::filled(BoundingBox { min, max }, test_state(fill))
                .expect("a test box fits in memory"),
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
        for y in self.blocks.bounds().min.y..=top {
            self.blocks.fill_layer(y, test_state(id));
        }
        self
    }

    /// Puts a block in the world without logging a write.
    pub fn seed(&mut self, at: BlockPos, voxel: VoxelId) {
        self.blocks.set_voxel(at, test_state(voxel));
    }

    pub fn seed_layer(&mut self, y: i32, voxel: VoxelId) {
        self.blocks.fill_layer(y, test_state(voxel));
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
    fn bounds(&self) -> BoundingBox {
        self.blocks.bounds()
    }

    fn access(&self, section: SectionPos) -> Access {
        self.blocks.access(section)
    }
}

impl VoxelStore for BoxRegion {
    fn voxel(&self, at: BlockPos) -> BlockStateId {
        self.blocks.voxel(at)
    }
}

impl VoxelStoreMut for BoxRegion {
    fn set_voxel(&mut self, at: BlockPos, state: BlockStateId) -> BlockStateId {
        if self.contains(at) {
            self.writes.push((at, VoxelId(state.number())));
        }
        self.blocks.set_voxel(at, state)
    }
}

impl WorldGenVolume for BoxRegion {
    fn world(&self) -> &WorldStates {
        &self.world
    }

    fn height(&self, kind: HeightmapName, x: i32, z: i32) -> i32 {
        (self.height)(&self.blocks, kind, x, z)
    }

    fn biome(&self, _: BlockPos) -> u16 {
        self.biome
    }

    fn extent(&self) -> HeightContext {
        self.extent
    }

    fn would_survive(&self, _: VoxelId, _: BlockPos) -> bool {
        true
    }
}
