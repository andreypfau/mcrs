use crate::world::generate::{ColumnBlocks, beta_chunk_seed};
use mcrs_minecraft_decoration::carver::cave::CaveWorldCarver;
use mcrs_minecraft_decoration::carver::config::BetaCaveCarverConfig;
use mcrs_minecraft_decoration::carver::mask::CarvingMask;
use mcrs_minecraft_decoration::carver::water::WaterMask;
use mcrs_minecraft_decoration::carver::{WorldCarver, can_replace_block};
use mcrs_minecraft_protocol::BlockStateId;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_voxel_storage::VoxelId;

pub struct BetaCaveBlockIds {
    pub air: BlockStateId,
    pub lava: BlockStateId,
    pub stone: BlockStateId,
    pub dirt: BlockStateId,
    pub grass: BlockStateId,
    pub water: BlockStateId,
    pub stationary_water: BlockStateId,
}

impl BetaCaveBlockIds {
    pub fn resolve(blocks: &BlockDefinitions) -> Self {
        BetaCaveBlockIds {
            air: blocks.default_state("minecraft:air"),
            lava: blocks.default_state("minecraft:lava"),
            stone: blocks.default_state("minecraft:stone"),
            dirt: blocks.default_state("minecraft:dirt"),
            grass: blocks.default_state("minecraft:grass_block"),
            // Both water and stationary_water map to the same modern water source state.
            // back2beta captures stationary water (ID 9) at sea-level fill positions;
            // the surface pass places water source state there, so we check the same ID.
            water: blocks.default_state("minecraft:water"),
            stationary_water: blocks.default_state("minecraft:water"),
        }
    }
}

/// Every water block of the column, as the carver's abort oracle.
///
/// Section cells are dense and in palette index order, so one linear scan per
/// section covers the whole column.
fn water_mask(column: &ColumnBlocks, ids: &BetaCaveBlockIds) -> WaterMask {
    let water: VoxelId = ids.water.into();
    let stationary: VoxelId = ids.stationary_water.into();
    let mut mask = WaterMask::default();
    for (slot, &section_y) in column.y_sections().iter().enumerate() {
        let base_y = section_y * 16;
        if base_y >= 128 || base_y + 16 <= 0 {
            continue;
        }
        for (index, cell) in column.section_cells(slot).iter().enumerate() {
            let state = cell.get();
            if state != water && state != stationary {
                continue;
            }
            mask.insert(
                (index % 16) as i32,
                base_y + (index / 256) as i32,
                ((index / 16) % 16) as i32,
            );
        }
    }
    mask
}

/// Beta's geometry Y range: the ellipsoid bounds clamp to `1..=119`, and the
/// block written for a marked Y sits one above it.
const MASK_MIN_Y: i32 = 1;
const MASK_MAX_Y: i32 = 119;

/// Fill the space the carver freed: lava under the lava level, air above it,
/// and dirt turned to grass directly under the first grass seen coming down.
///
/// One pass per carved run, top down, which is where the grass fixup has to
/// look: the block it converts is the next Y the same run visits.
fn apply_cave_substance(
    column: &ColumnBlocks,
    mask: &CarvingMask,
    config: &BetaCaveCarverConfig,
    ids: &BetaCaveBlockIds,
) {
    let air: VoxelId = ids.air.into();
    mask.visit(|x, z, bottom_y, top_y| {
        let mut has_grass = false;
        for y in (bottom_y..=top_y).rev() {
            let cell_y = y + 1;
            let state = column.get(x, cell_y, z).unwrap_or(air);
            if state == config.grass_state {
                has_grass = true;
            }
            if !can_replace_block(config, state) {
                continue;
            }
            if y < config.lava_level {
                column.set(x, cell_y, z, config.lava_state);
            } else {
                column.set(x, cell_y, z, config.air_state);
                if has_grass && column.get(x, y, z) == Some(config.dirt_state) {
                    column.set(x, y, z, config.grass_state);
                }
            }
        }
    });
}

pub fn apply_beta_caves(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    config: &BetaCaveCarverConfig,
    ids: &BetaCaveBlockIds,
) {
    let carver = CaveWorldCarver;
    let water = water_mask(column, ids);
    let mut mask = CarvingMask::new(16, MASK_MIN_Y, MASK_MAX_Y);

    let radius = config.range;
    for origin_x in (chunk_x - radius)..=(chunk_x + radius) {
        for origin_z in (chunk_z - radius)..=(chunk_z + radius) {
            let mut carve_rng =
                LegacyRandom::new(beta_chunk_seed(world_seed, origin_x, origin_z) as u64);

            carver.carve(
                config,
                chunk_x,
                chunk_z,
                origin_x,
                origin_z,
                &water,
                &mut mask,
                &mut carve_rng,
            );
        }
    }

    apply_cave_substance(column, &mask, config, ids);
}
