use crate::world::generate::{ColumnBlocks, beta_chunk_seed};
use mcrs_minecraft_decoration::carver::WorldCarver;
use mcrs_minecraft_decoration::carver::cave::CaveWorldCarver;
use mcrs_minecraft_decoration::carver::config::BetaCaveCarverConfig;
use mcrs_minecraft_decoration::carver::water::WaterMask;
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

    let air: VoxelId = ids.air.into();
    let get_block = |local_x: i32, world_y: i32, local_z: i32| -> VoxelId {
        column.get(local_x, world_y, local_z).unwrap_or(air)
    };
    let set_block = |local_x: i32, world_y: i32, local_z: i32, state: VoxelId| {
        column.set(local_x, world_y, local_z, state);
    };

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
                &get_block,
                &set_block,
                &mut carve_rng,
            );
        }
    }
}
