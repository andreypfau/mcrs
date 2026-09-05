#![allow(dead_code)]

use bevy_ecs::prelude::Entity;

use std::sync::{Arc, OnceLock};

use bevy_math::Vec3;
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_math::voxel_shape::{Aabb, VoxelShape};
use mcrs_voxel_math::{BlockPos, ChunkPos, Direction};
use mcrs_voxel_storage::{PalettedContainer, VoxelId, VoxelPalette};

pub const AIR: VoxelId = VoxelId(0);
pub const STONE: VoxelId = VoxelId(1);
pub const GLASS: VoxelId = VoxelId(2);
pub const WATER: VoxelId = VoxelId(3);
pub const LEAVES: VoxelId = VoxelId(4);
pub const TINTED_GLASS: VoxelId = VoxelId(5);
pub const TORCH: VoxelId = VoxelId(6);
pub const GLOWSTONE: VoxelId = VoxelId(7);
pub const BOTTOM_SLAB: VoxelId = VoxelId(8);
pub const TOP_SLAB: VoxelId = VoxelId(9);

const LOWER_HALF: Aabb = Aabb {
    min: Vec3::ZERO,
    max: Vec3::new(1.0, 0.5, 1.0),
};
const UPPER_HALF: Aabb = Aabb {
    min: Vec3::new(0.0, 0.5, 0.0),
    max: Vec3::ONE,
};

fn slab_shape(aabb: Aabb) -> &'static VoxelShape {
    Box::leak(Box::new(VoxelShape::from_boxes(&[aabb])))
}

fn bottom_slab_shape() -> &'static VoxelShape {
    static SHAPE: OnceLock<&'static VoxelShape> = OnceLock::new();
    *SHAPE.get_or_init(|| slab_shape(LOWER_HALF))
}

fn top_slab_shape() -> &'static VoxelShape {
    static SHAPE: OnceLock<&'static VoxelShape> = OnceLock::new();
    *SHAPE.get_or_init(|| slab_shape(UPPER_HALF))
}

pub fn filled(block: VoxelId) -> SectionBlocks {
    VoxelPalette(PalettedContainer::Homogeneous(block))
}

pub fn registry() -> Arc<LightRegistry> {
    Arc::new(LightRegistry::new(
        vec![
            LightProperties::AIR,
            LightProperties::SOLID,
            // Glass is transparent to light and, crucially, does not end a run
            // of sky sources.
            LightProperties::transparent(0),
            // Water and leaves dim by one, which is invisible to block light but
            // does end the sky column.
            LightProperties::transparent(1),
            LightProperties::transparent(1),
            LightProperties::transparent(15),
            LightProperties::emitter(14),
            LightProperties::emitter(15),
            LightProperties::shaped(0, bottom_slab_shape()),
            LightProperties::shaped(0, top_slab_shape()),
        ],
        SpecialBlocks {
            unloaded: STONE,
            outside: AIR,
        },
    ))
}

pub fn offset(pos: BlockPos, dir: Direction) -> BlockPos {
    let n = dir.normal();
    BlockPos::new(pos.x + n.x, pos.y + n.y, pos.z + n.z)
}

pub struct TestWorld {
    pub world: LightWorld,
    pub sections: (i32, i32, i32),
}

impl TestWorld {
    pub fn new(sections_x: i32, sections_y: i32, sections_z: i32) -> Self {
        let bounds = LightBounds::new(0, sections_y - 1);
        let mut world = LightWorld::new(registry(), bounds);
        let loads: Vec<Edit> = (0..sections_y)
            .flat_map(|y| {
                (0..sections_z).flat_map(move |z| {
                    (0..sections_x).map(move |x| Edit::LoadSection {
                        entity: Entity::PLACEHOLDER,
                        pos: ChunkPos::new(x, y, z),
                        blocks: Arc::new(filled(AIR)),
                    })
                })
            })
            .collect();
        world.update_now(loads);
        Self {
            world,
            sections: (sections_x, sections_y, sections_z),
        }
    }

    pub fn min(&self) -> BlockPos {
        BlockPos::new(0, 0, 0)
    }

    pub fn max(&self) -> BlockPos {
        BlockPos::new(
            self.sections.0 * 16 - 1,
            self.sections.1 * 16 - 1,
            self.sections.2 * 16 - 1,
        )
    }

    pub fn set(&mut self, pos: BlockPos, block: VoxelId) -> EpochStats {
        self.world.update_now([Edit::SetBlock { pos, block }])
    }

    pub fn set_many(&mut self, edits: impl IntoIterator<Item = (BlockPos, VoxelId)>) -> EpochStats {
        let edits: Vec<Edit> = edits
            .into_iter()
            .map(|(pos, block)| Edit::SetBlock { pos, block })
            .collect();
        self.world.update_now(edits)
    }

    /// Walls a single column in with stone between `top` and `bottom`, so that
    /// light reaching the cells inside can only have come down the column.
    /// Without this, an open world lights every cell from the side and a
    /// vertical profile says nothing.
    pub fn build_shaft(&mut self, x: i32, z: i32, top: i32, bottom: i32) {
        let mut walls = Vec::new();
        for y in bottom..=top {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if (dx, dz) != (0, 0) {
                        walls.push((BlockPos::new(x + dx, y, z + dz), STONE));
                    }
                }
            }
        }
        self.set_many(walls);
    }

    pub fn block_light(&self, pos: BlockPos) -> u8 {
        self.world.light_at(pos, Layer::Block).get()
    }

    pub fn sky_light(&self, pos: BlockPos) -> u8 {
        self.world.light_at(pos, Layer::Sky).get()
    }

    /// Sky light going down a column, from `top` to `bottom` inclusive.
    pub fn sky_profile(&self, x: i32, z: i32, top: i32, bottom: i32) -> Vec<u8> {
        (bottom..=top)
            .rev()
            .map(|y| self.sky_light(BlockPos::new(x, y, z)))
            .collect()
    }

    pub fn check_against_reference(&self) {
        let reference = Reference::compute(&self.world, self.min(), self.max());
        if let Some(diff) = reference.diff(&self.world) {
            panic!("engine disagrees with the reference solver: {diff}");
        }
    }
}

fn flat_index(min: BlockPos, dim: (i32, i32, i32), p: BlockPos) -> usize {
    (((p.y - min.y) * dim.2 + (p.z - min.z)) * dim.0 + (p.x - min.x)) as usize
}

/// An independent solver used to check the incremental engine.
///
/// It reuses the block rules — those are pinned by the targeted tests — but
/// finds the answer by recomputing every cell from its neighbours until nothing
/// moves. It shares no code with the frontier relaxation, the influence boxes
/// or the erase phase.
pub struct Reference {
    min: BlockPos,
    dim: (i32, i32, i32),
    pub block: Vec<u8>,
    pub sky: Vec<u8>,
}

impl Reference {
    pub fn compute(world: &LightWorld, min: BlockPos, max: BlockPos) -> Self {
        let registry = world.registry().clone();
        let dim = (max.x - min.x + 1, max.y - min.y + 1, max.z - min.z + 1);
        let count = (dim.0 * dim.1 * dim.2) as usize;
        let idx = |p: BlockPos| flat_index(min, dim, p);
        let inside = |p: BlockPos| {
            p.x >= min.x
                && p.x <= max.x
                && p.y >= min.y
                && p.y <= max.y
                && p.z >= min.z
                && p.z <= max.z
        };

        let top = world.bounds().max_light_y();
        let mut sky_source = vec![false; count];
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                for x in min.x..=max.x {
                    let mut open = true;
                    for scan_y in y..top {
                        let below = world.block_at(BlockPos::new(x, scan_y, z));
                        let above = world.block_at(BlockPos::new(x, scan_y + 1, z));
                        if registry.breaks_sky_column(above, below) {
                            open = false;
                            break;
                        }
                    }
                    sky_source[idx(BlockPos::new(x, y, z))] = open;
                }
            }
        }

        let mut block = vec![0u8; count];
        let mut sky = vec![0u8; count];
        loop {
            let mut changed = false;
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    for x in min.x..=max.x {
                        let p = BlockPos::new(x, y, z);
                        let i = idx(p);
                        let here = world.block_at(p);
                        let mut best_block = registry.emission(here, Layer::Block).get();
                        let mut best_sky = if sky_source[i] { 15 } else { 0 };

                        for dir in Direction::all() {
                            let q = offset(p, dir);
                            if !inside(q) {
                                continue;
                            }
                            let neighbour = world.block_at(q);
                            // Travelling from q into p goes the opposite way.
                            let Some(cost) = registry.attenuation(neighbour, here, dir.opposite())
                            else {
                                continue;
                            };
                            let j = idx(q);
                            best_block = best_block.max(block[j].saturating_sub(cost));
                            best_sky = best_sky.max(sky[j].saturating_sub(cost));
                        }

                        if best_block > block[i] {
                            block[i] = best_block;
                            changed = true;
                        }
                        if best_sky > sky[i] {
                            sky[i] = best_sky;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }

        Self {
            min,
            dim,
            block,
            sky,
        }
    }

    fn index(&self, p: BlockPos) -> usize {
        flat_index(self.min, self.dim, p)
    }

    pub fn diff(&self, world: &LightWorld) -> Option<String> {
        for y in self.min.y..self.min.y + self.dim.1 {
            for z in self.min.z..self.min.z + self.dim.2 {
                for x in self.min.x..self.min.x + self.dim.0 {
                    let p = BlockPos::new(x, y, z);
                    let i = self.index(p);
                    let got_block = world.light_at(p, Layer::Block).get();
                    let got_sky = world.light_at(p, Layer::Sky).get();
                    if got_block != self.block[i] || got_sky != self.sky[i] {
                        return Some(format!(
                            "at {p:?}: engine block={got_block} sky={got_sky}, \
                             reference block={} sky={}",
                            self.block[i], self.sky[i]
                        ));
                    }
                }
            }
        }
        None
    }
}
