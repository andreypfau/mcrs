use mcrs_voxel_math::BlockPos;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_worldgen::carver::WorldCarver;
use mcrs_minecraft_worldgen::carver::cave::CaveWorldCarver;
use mcrs_minecraft_worldgen::carver::config::BetaCaveCarverConfig;
use mcrs_protocol::BlockStateId;
use mcrs_random::legacy::LegacyRandom;
use mcrs_vanilla::block::definition::BlockDefinitions;
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

fn get_block_from_sections(
    sections: &[Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    local_x: i32,
    world_y: i32,
    local_z: i32,
    air: BlockStateId,
) -> BlockStateId {
    let section_y = world_y >> 4;
    let local_y = world_y & 0xF;
    if let Some(si) = y_sections.iter().position(|&sy| sy == section_y)
        && let Some(Some((blocks, _))) = sections.get(si) {
            return blocks.get(BlockPos::new(local_x, local_y, local_z)).into();
        }
    air
}

fn set_block_in_sections(
    sections: &mut [Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    local_x: i32,
    world_y: i32,
    local_z: i32,
    state: BlockStateId,
) {
    let section_y = world_y >> 4;
    let local_y = world_y & 0xF;
    if let Some(si) = y_sections.iter().position(|&sy| sy == section_y)
        && let Some(Some((blocks, _))) = sections.get_mut(si) {
            blocks.set(BlockPos::new(local_x, local_y, local_z), state.into());
        }
}

pub fn apply_beta_caves(
    sections: &mut Vec<Option<(BlockPalette, BiomePalette)>>,
    y_sections: &[i32],
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

    // SAFETY: get_block reads from sections while set_block writes to sections.
    // The CaveWorldCarver::carve implementation never calls both closures
    // concurrently — it calls one at a time in a single-threaded loop.
    // The pointer is valid for the duration of the carve call, which does not
    // outlive `sections`.
    let sections_ptr = sections.as_mut_slice() as *mut [Option<(BlockPalette, BiomePalette)>];

    let radius = config.range;
    for origin_x in (chunk_x - radius)..=(chunk_x + radius) {
        for origin_z in (chunk_z - radius)..=(chunk_z + radius) {
            let seed: i64 = (origin_x as i64)
                .wrapping_mul(l)
                .wrapping_add((origin_z as i64).wrapping_mul(i1))
                ^ world_seed;
            let mut carve_rng = LegacyRandom::new(seed as u64);

            let air = ids.air;
            let get_block = |local_x: i32, world_y: i32, local_z: i32| -> VoxelId {
                let slice = unsafe { &*sections_ptr };
                get_block_from_sections(slice, y_sections, local_x, world_y, local_z, air).into()
            };

            let set_block = |local_x: i32, world_y: i32, local_z: i32, state: VoxelId| {
                let slice = unsafe { &mut *sections_ptr };
                set_block_in_sections(
                    slice,
                    y_sections,
                    local_x,
                    world_y,
                    local_z,
                    state.into(),
                );
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
