use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_worldgen::feature::OreFeature;
use mcrs_minecraft_worldgen::feature::config::{OreConfig, OreYOffset, TargetBlockState};
use mcrs_protocol::BlockStateId;
use mcrs_random::legacy::LegacyRandom;
use mcrs_random::Random;
use mcrs_vanilla::block::minecraft;

pub struct BetaOreBlockIds {
    pub stone: BlockStateId,
    pub clay: BlockStateId,
    pub dirt: BlockStateId,
    pub gravel: BlockStateId,
    pub coal: BlockStateId,
    pub iron: BlockStateId,
    pub gold: BlockStateId,
    pub redstone: BlockStateId,
    pub diamond: BlockStateId,
    pub lapis: BlockStateId,
    pub water: BlockStateId,
}

impl BetaOreBlockIds {
    pub fn resolve() -> Self {
        BetaOreBlockIds {
            stone: minecraft::STONE.default_state_id,
            clay: minecraft::CLAY.default_state_id,
            dirt: minecraft::DIRT.default_state_id,
            gravel: minecraft::GRAVEL.default_state_id,
            coal: minecraft::COAL_ORE.default_state_id,
            iron: minecraft::IRON_ORE.default_state_id,
            gold: minecraft::GOLD_ORE.default_state_id,
            // REDSTONE_ORE default state carries lit=false, matching Beta placement
            redstone: minecraft::REDSTONE_ORE.default_state_id,
            diamond: minecraft::DIAMOND_ORE.default_state_id,
            lapis: minecraft::LAPIS_ORE.default_state_id,
            water: minecraft::WATER.default_state_id,
        }
    }
}

fn ore_config(stone: BlockStateId, state: BlockStateId, size: i32) -> OreConfig {
    OreConfig {
        targets: vec![TargetBlockState { target: stone, state }],
        size,
        y_offset: OreYOffset::BetaPlus2,
    }
}

fn get_block_from_sections(
    sections: &[Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    world_x: i32,
    world_y: i32,
    world_z: i32,
    chunk_x: i32,
    chunk_z: i32,
) -> BlockStateId {
    let local_x = world_x - chunk_x * 16;
    let local_z = world_z - chunk_z * 16;
    if local_x < 0 || local_x >= 16 || local_z < 0 || local_z >= 16 || world_y < 0 {
        return BlockStateId(0);
    }
    let section_y = world_y >> 4;
    let local_y = world_y & 0xF;
    if let Some(si) = y_sections.iter().position(|&sy| sy == section_y) {
        if let Some(Some((blocks, _))) = sections.get(si) {
            return blocks.get(BlockPos::new(local_x, local_y, local_z));
        }
    }
    BlockStateId(0)
}

fn set_block_in_sections(
    sections: &mut [Option<(BlockPalette, BiomePalette)>],
    y_sections: &[i32],
    world_x: i32,
    world_y: i32,
    world_z: i32,
    chunk_x: i32,
    chunk_z: i32,
    state: BlockStateId,
) {
    let local_x = world_x - chunk_x * 16;
    let local_z = world_z - chunk_z * 16;
    if local_x < 0 || local_x >= 16 || local_z < 0 || local_z >= 16 || world_y < 0 {
        return;
    }
    let section_y = world_y >> 4;
    let local_y = world_y & 0xF;
    if let Some(si) = y_sections.iter().position(|&sy| sy == section_y) {
        if let Some(Some((blocks, _))) = sections.get_mut(si) {
            blocks.set(BlockPos::new(local_x, local_y, local_z), state);
        }
    }
}

fn place_ore<R: Random>(
    feature: &OreFeature,
    config: &OreConfig,
    count: i32,
    y_bound: i32,
    chunk_x: i32,
    chunk_z: i32,
    sections: &mut Vec<Option<(BlockPalette, BiomePalette)>>,
    y_sections: &[i32],
    rng: &mut R,
) {
    // SAFETY: get_block reads from sections while set_block writes — never concurrent.
    let sections_ptr = sections.as_mut_slice() as *mut [Option<(BlockPalette, BiomePalette)>];

    for _ in 0..count {
        let origin_x = chunk_x * 16 + rng.next_i32_bound(16);
        let origin_y = rng.next_i32_bound(y_bound);
        let origin_z = chunk_z * 16 + rng.next_i32_bound(16);

        let cx = chunk_x;
        let cz = chunk_z;

        let get_block = |wx: i32, wy: i32, wz: i32| -> BlockStateId {
            let sl = unsafe { &*sections_ptr };
            get_block_from_sections(sl, y_sections, wx, wy, wz, cx, cz)
        };
        let set_block = |wx: i32, wy: i32, wz: i32, state: BlockStateId| {
            let sl = unsafe { &mut *sections_ptr };
            set_block_in_sections(sl, y_sections, wx, wy, wz, cx, cz, state);
        };

        feature.place(config, origin_x, origin_y, origin_z, get_block, set_block, rng);
    }
}

fn place_clay<R: Random>(
    count: i32,
    chunk_x: i32,
    chunk_z: i32,
    ids: &BetaOreBlockIds,
    sections: &mut Vec<Option<(BlockPalette, BiomePalette)>>,
    y_sections: &[i32],
    rng: &mut R,
) {
    // Clay (WorldGenClay, size 32): check y-1 for water before placing
    let config = ore_config(ids.stone, ids.clay, 32);
    let feature = OreFeature;

    let sections_ptr = sections.as_mut_slice() as *mut [Option<(BlockPalette, BiomePalette)>];
    let cx = chunk_x;
    let cz = chunk_z;
    let water = ids.water;

    for _ in 0..count {
        let origin_x = chunk_x * 16 + rng.next_i32_bound(16);
        let origin_y = rng.next_i32_bound(128);
        let origin_z = chunk_z * 16 + rng.next_i32_bound(16);

        // Beta's WorldGenClay only places clay in shallow-water contexts.
        // Check whether water is present at y-1 (below the origin) as a proxy.
        let below_state = {
            let sl = unsafe { &*sections_ptr };
            get_block_from_sections(sl, y_sections, origin_x, origin_y - 1, origin_z, cx, cz)
        };
        if below_state != water {
            continue;
        }

        let get_block = |wx: i32, wy: i32, wz: i32| -> BlockStateId {
            let sl = unsafe { &*sections_ptr };
            get_block_from_sections(sl, y_sections, wx, wy, wz, cx, cz)
        };
        let set_block = |wx: i32, wy: i32, wz: i32, state: BlockStateId| {
            let sl = unsafe { &mut *sections_ptr };
            set_block_in_sections(sl, y_sections, wx, wy, wz, cx, cz, state);
        };

        feature.place(&config, origin_x, origin_y, origin_z, get_block, set_block, rng);
    }
}

/// Derive Beta populate seed and place all nine ore/terrain resource types in Beta order.
///
/// Populate seed formula mirrors ChunkProviderGenerate.getChunkAt lines 317–320.
/// The pre-ore lake and dungeon draws are skipped; vein positions diverge from the
/// reference as a result, so distribution (count + Y-range) is the parity target here.
pub fn apply_beta_ores(
    sections: &mut Vec<Option<(BlockPalette, BiomePalette)>>,
    y_sections: &[i32],
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    ids: &BetaOreBlockIds,
) {
    let mut seed_rng = LegacyRandom::new(world_seed as u64);
    let i1: i64 = seed_rng.next_java_long() / 2 * 2 + 1;
    let j1: i64 = seed_rng.next_java_long() / 2 * 2 + 1;
    let populate_seed: i64 = (chunk_x as i64)
        .wrapping_mul(i1)
        .wrapping_add((chunk_z as i64).wrapping_mul(j1))
        ^ world_seed;
    let mut rng = LegacyRandom::new(populate_seed as u64);
    place_all_ores(sections, y_sections, chunk_x, chunk_z, &mut rng, ids);
}

/// Place all nine resource types in Beta order using an externally-supplied RNG
/// (already seeded with the populate seed). Split from `apply_beta_ores` so the
/// distribution test can drive it with an instrumented RNG to pin the draw count.
pub fn place_all_ores<R: Random>(
    sections: &mut Vec<Option<(BlockPalette, BiomePalette)>>,
    y_sections: &[i32],
    chunk_x: i32,
    chunk_z: i32,
    rng: &mut R,
    ids: &BetaOreBlockIds,
) {
    let feature = OreFeature;
    let stone = ids.stone;

    // Beta placement order from ChunkProviderGenerate.getChunkAt lines 344–406:

    // Clay 10×32, Y<128 — water-adjacent check applied
    place_clay(10, chunk_x, chunk_z, ids, sections, y_sections, rng);

    // Dirt 20×32, Y<128
    let dirt_cfg = ore_config(stone, ids.dirt, 32);
    place_ore(&feature, &dirt_cfg, 20, 128, chunk_x, chunk_z, sections, y_sections, rng);

    // Gravel 10×32, Y<128
    let gravel_cfg = ore_config(stone, ids.gravel, 32);
    place_ore(&feature, &gravel_cfg, 10, 128, chunk_x, chunk_z, sections, y_sections, rng);

    // Coal 20×16, Y<128
    let coal_cfg = ore_config(stone, ids.coal, 16);
    place_ore(&feature, &coal_cfg, 20, 128, chunk_x, chunk_z, sections, y_sections, rng);

    // Iron 20×8, Y<64
    let iron_cfg = ore_config(stone, ids.iron, 8);
    place_ore(&feature, &iron_cfg, 20, 64, chunk_x, chunk_z, sections, y_sections, rng);

    // Gold 2×8, Y<32
    let gold_cfg = ore_config(stone, ids.gold, 8);
    place_ore(&feature, &gold_cfg, 2, 32, chunk_x, chunk_z, sections, y_sections, rng);

    // Redstone 8×7, Y<16
    let redstone_cfg = ore_config(stone, ids.redstone, 7);
    place_ore(&feature, &redstone_cfg, 8, 16, chunk_x, chunk_z, sections, y_sections, rng);

    // Diamond 1×7, Y<16
    let diamond_cfg = ore_config(stone, ids.diamond, 7);
    place_ore(&feature, &diamond_cfg, 1, 16, chunk_x, chunk_z, sections, y_sections, rng);

    // Lapis 1×6, draw order x, then Y = nextInt(16) + nextInt(16), then z (Beta order)
    let lapis_origin_x = chunk_x * 16 + rng.next_i32_bound(16);
    let lapis_origin_y = rng.next_i32_bound(16) + rng.next_i32_bound(16);
    let lapis_origin_z = chunk_z * 16 + rng.next_i32_bound(16);
    let lapis_cfg = ore_config(stone, ids.lapis, 6);

    let sections_ptr = sections.as_mut_slice() as *mut [Option<(BlockPalette, BiomePalette)>];
    let cx = chunk_x;
    let cz = chunk_z;
    let ys = y_sections;

    let get_block = |wx: i32, wy: i32, wz: i32| -> BlockStateId {
        let sl = unsafe { &*sections_ptr };
        get_block_from_sections(sl, ys, wx, wy, wz, cx, cz)
    };
    let set_block = |wx: i32, wy: i32, wz: i32, state: BlockStateId| {
        let sl = unsafe { &mut *sections_ptr };
        set_block_in_sections(sl, ys, wx, wy, wz, cx, cz, state);
    };
    feature.place(&lapis_cfg, lapis_origin_x, lapis_origin_y, lapis_origin_z, get_block, set_block, rng);
}
