use crate::world::generate::{ColumnBlocks, beta_chunk_seed};
use mcrs_minecraft_decoration::feature::OreFeature;
use mcrs_minecraft_decoration::feature::config::{OreConfig, OreYOffset, TargetBlockState};
use mcrs_minecraft_protocol::BlockStateId;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_voxel_storage::VoxelId;

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
    pub fn resolve(blocks: &BlockDefinitions) -> Self {
        BetaOreBlockIds {
            stone: blocks.default_state("minecraft:stone"),
            clay: blocks.default_state("minecraft:clay"),
            dirt: blocks.default_state("minecraft:dirt"),
            gravel: blocks.default_state("minecraft:gravel"),
            coal: blocks.default_state("minecraft:coal_ore"),
            iron: blocks.default_state("minecraft:iron_ore"),
            gold: blocks.default_state("minecraft:gold_ore"),
            // REDSTONE_ORE default state carries lit=false, matching Beta placement
            redstone: blocks.default_state("minecraft:redstone_ore"),
            diamond: blocks.default_state("minecraft:diamond_ore"),
            lapis: blocks.default_state("minecraft:lapis_ore"),
            water: blocks.default_state("minecraft:water"),
        }
    }
}

fn ore_config(stone: BlockStateId, state: BlockStateId, size: i32) -> OreConfig {
    OreConfig {
        targets: vec![TargetBlockState {
            target: stone.into(),
            state: state.into(),
        }],
        size,
        y_offset: OreYOffset::BetaPlus2,
    }
}

/// The column seen in world coordinates: a reader, a writer, and the xz box the
/// vein is worth scanning. Blocks outside the box were dropped by the writer.
fn chunk_view(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
) -> (
    impl Fn(i32, i32, i32) -> VoxelId + '_,
    impl Fn(i32, i32, i32, VoxelId) + '_,
    (i32, i32, i32, i32),
) {
    let (ox, oz) = (chunk_x * 16, chunk_z * 16);
    (
        move |wx, wy, wz| column.get(wx - ox, wy, wz - oz).unwrap_or_default(),
        move |wx, wy, wz, state| column.set(wx - ox, wy, wz - oz, state),
        (ox, ox + 15, oz, oz + 15),
    )
}

fn place_ore<R: Random>(
    feature: &OreFeature,
    config: &OreConfig,
    count: i32,
    y_bound: i32,
    chunk_x: i32,
    chunk_z: i32,
    column: &ColumnBlocks,
    rng: &mut R,
) {
    let (get_block, set_block, bounds) = chunk_view(column, chunk_x, chunk_z);

    for _ in 0..count {
        let origin_x = chunk_x * 16 + rng.next_i32_bound(16);
        let origin_y = rng.next_i32_bound(y_bound);
        let origin_z = chunk_z * 16 + rng.next_i32_bound(16);

        feature.place_within(
            config,
            origin_x,
            origin_y,
            origin_z,
            Some(bounds),
            &get_block,
            &set_block,
            rng,
        );
    }
}

fn place_clay<R: Random>(
    count: i32,
    chunk_x: i32,
    chunk_z: i32,
    ids: &BetaOreBlockIds,
    column: &ColumnBlocks,
    rng: &mut R,
) {
    // Clay (WorldGenClay, size 32): check y-1 for water before placing
    let config = ore_config(ids.stone, ids.clay, 32);
    let feature = OreFeature;

    let water = ids.water;
    let (get_block, set_block, bounds) = chunk_view(column, chunk_x, chunk_z);

    for _ in 0..count {
        let origin_x = chunk_x * 16 + rng.next_i32_bound(16);
        let origin_y = rng.next_i32_bound(128);
        let origin_z = chunk_z * 16 + rng.next_i32_bound(16);

        // Beta's WorldGenClay only places clay in shallow-water contexts.
        // Check whether water is present at y-1 (below the origin) as a proxy.
        let below_state: BlockStateId = get_block(origin_x, origin_y - 1, origin_z).into();
        if below_state != water {
            continue;
        }

        feature.place_within(
            &config,
            origin_x,
            origin_y,
            origin_z,
            Some(bounds),
            &get_block,
            &set_block,
            rng,
        );
    }
}

/// Derive Beta populate seed and place all nine ore/terrain resource types in Beta order.
///
/// Populate seed formula mirrors ChunkProviderGenerate.getChunkAt lines 317–320.
/// The pre-ore lake and dungeon draws are skipped; vein positions diverge from the
/// reference as a result, so distribution (count + Y-range) is the parity target here.
pub fn apply_beta_ores(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    ids: &BetaOreBlockIds,
) {
    let mut rng = LegacyRandom::new(beta_chunk_seed(world_seed, chunk_x, chunk_z) as u64);
    place_all_ores(column, chunk_x, chunk_z, &mut rng, ids);
}

/// Place all nine resource types in Beta order using an externally-supplied RNG
/// (already seeded with the populate seed). Split from `apply_beta_ores` so the
/// distribution test can drive it with an instrumented RNG to pin the draw count.
pub fn place_all_ores<R: Random>(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
    rng: &mut R,
    ids: &BetaOreBlockIds,
) {
    let feature = OreFeature;
    let stone = ids.stone;

    // Beta placement order from ChunkProviderGenerate.getChunkAt lines 344–406,
    // as (block, count, vein size, Y bound). Clay leads, and is drawn apart from
    // the rest because it only settles under water.
    place_clay(10, chunk_x, chunk_z, ids, column, rng);

    for (state, count, size, y_bound) in [
        (ids.dirt, 20, 32, 128),
        (ids.gravel, 10, 32, 128),
        (ids.coal, 20, 16, 128),
        (ids.iron, 20, 8, 64),
        (ids.gold, 2, 8, 32),
        (ids.redstone, 8, 7, 16),
        (ids.diamond, 1, 7, 16),
    ] {
        let config = ore_config(stone, state, size);
        place_ore(
            &feature, &config, count, y_bound, chunk_x, chunk_z, column, rng,
        );
    }

    // Lapis 1×6, draw order x, then Y = nextInt(16) + nextInt(16), then z (Beta order)
    let lapis_origin_x = chunk_x * 16 + rng.next_i32_bound(16);
    let lapis_origin_y = rng.next_i32_bound(16) + rng.next_i32_bound(16);
    let lapis_origin_z = chunk_z * 16 + rng.next_i32_bound(16);
    let lapis_cfg = ore_config(stone, ids.lapis, 6);

    let (get_block, set_block, bounds) = chunk_view(column, chunk_x, chunk_z);
    feature.place_within(
        &lapis_cfg,
        lapis_origin_x,
        lapis_origin_y,
        lapis_origin_z,
        Some(bounds),
        &get_block,
        &set_block,
        rng,
    );
}
