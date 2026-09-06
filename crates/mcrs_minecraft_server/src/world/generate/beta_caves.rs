use crate::world::generate::ColumnBlocks;
use mcrs_minecraft_decoration::carver::WorldCarver;
use mcrs_minecraft_decoration::carver::cave::CaveWorldCarver;
use mcrs_minecraft_decoration::carver::config::BetaCaveCarverConfig;
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

pub fn apply_beta_caves(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    config: &BetaCaveCarverConfig,
    ids: &BetaCaveBlockIds,
) {
    let carver = CaveWorldCarver;

    let mut seed_rng = LegacyRandom::new(world_seed as u64);
    let l: i64 = seed_rng.next_java_long() / 2 * 2 + 1;
    let i1: i64 = seed_rng.next_java_long() / 2 * 2 + 1;

    let radius = config.range;
    for origin_x in (chunk_x - radius)..=(chunk_x + radius) {
        for origin_z in (chunk_z - radius)..=(chunk_z + radius) {
            let seed: i64 = (origin_x as i64)
                .wrapping_mul(l)
                .wrapping_add((origin_z as i64).wrapping_mul(i1))
                ^ world_seed;
            let mut carve_rng = LegacyRandom::new(seed as u64);

            let air: VoxelId = ids.air.into();
            let get_block = |local_x: i32, world_y: i32, local_z: i32| -> VoxelId {
                column.get(local_x, world_y, local_z).unwrap_or(air)
            };

            let set_block = |local_x: i32, world_y: i32, local_z: i32, state: VoxelId| {
                column.set(local_x, world_y, local_z, state);
            };

            carver.carve(
                config,
                chunk_x,
                chunk_z,
                origin_x,
                origin_z,
                get_block,
                set_block,
                &mut carve_rng,
            );
        }
    }
}
