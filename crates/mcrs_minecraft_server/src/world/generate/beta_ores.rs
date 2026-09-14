use mcrs_minecraft_core::BlockPos;

use crate::world::generate::beta_chunk_seed;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::BlocksMut;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_worldgen_feature_place::ore_beta::{
    OreConfig, TargetBlockState, place_beta_ore,
};

pub struct BetaOreBlockIds {
    pub stone: BlockStateId,
    pub sand: BlockStateId,
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
            sand: blocks.default_state("minecraft:sand"),
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
    }
}

fn place_ore<R: Random>(
    config: &OreConfig,
    count: i32,
    y_bound: i32,
    origin_x: i32,
    origin_z: i32,
    volume: &mut impl BlocksMut,
    rng: &mut R,
) {
    for _ in 0..count {
        let x = origin_x + rng.next_i32_bound(16);
        let y = rng.next_i32_bound(y_bound);
        let z = origin_z + rng.next_i32_bound(16);
        place_beta_ore(config, BlockPos::new(x, y, z), volume, rng);
    }
}

fn place_clay<R: Random>(
    count: i32,
    origin_x: i32,
    origin_z: i32,
    ids: &BetaOreBlockIds,
    volume: &mut impl BlocksMut,
    rng: &mut R,
) {
    for _ in 0..count {
        let x = origin_x + rng.next_i32_bound(16);
        let y = rng.next_i32_bound(128);
        let z = origin_z + rng.next_i32_bound(16);
        place_clay_vein(BlockPos::new(x, y, z), ids, volume, rng);
    }
}

/// `WorldGenClay`: a vein of the ore shape that turns sand to clay, started
/// only from a water block, and drawing nothing when it does not start.
fn place_clay_vein<R: Random>(
    origin: BlockPos,
    ids: &BetaOreBlockIds,
    volume: &mut impl BlocksMut,
    rng: &mut R,
) {
    if BlockStateId::from(volume.get(origin)) != ids.water {
        return;
    }
    place_beta_ore(&ore_config(ids.sand, ids.clay, 32), origin, volume, rng);
}

/// Beta's populate step for one column against the region it runs in, so the
/// half of a vein that crosses the border survives.
///
/// Populate seed formula mirrors ChunkProviderGenerate.getChunkAt lines 317–320.
/// The pre-ore lake and dungeon draws are skipped; vein positions diverge from the
/// reference as a result, so distribution (count + Y-range) is the parity target here.
pub fn apply_beta_ores_in(
    volume: &mut impl BlocksMut,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    ids: &BetaOreBlockIds,
) {
    let mut rng = LegacyRandom::new(beta_chunk_seed(world_seed, chunk_x, chunk_z) as u64);
    place_all_ores(volume, chunk_x, chunk_z, &mut rng, ids);
}

/// Place all nine resource types in Beta order using an externally-supplied RNG
/// (already seeded with the populate seed). Split from `apply_beta_ores_in` so the
/// distribution test can drive it with an instrumented RNG to pin the draw count.
pub fn place_all_ores<R: Random>(
    volume: &mut impl BlocksMut,
    chunk_x: i32,
    chunk_z: i32,
    rng: &mut R,
    ids: &BetaOreBlockIds,
) {
    let stone = ids.stone;
    let origin_x = chunk_x * 16;
    let origin_z = chunk_z * 16;

    // Beta placement order from ChunkProviderGenerate.getChunkAt lines 344–406,
    // as (block, count, vein size, Y bound). Clay leads, and is drawn apart from
    // the rest because it only settles under water.
    place_clay(10, origin_x, origin_z, ids, volume, rng);

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
        place_ore(&config, count, y_bound, origin_x, origin_z, volume, rng);
    }

    // Lapis 1×6, draw order x, then Y = nextInt(16) + nextInt(16), then z (Beta order)
    let lapis_x = origin_x + rng.next_i32_bound(16);
    let lapis_y = rng.next_i32_bound(16) + rng.next_i32_bound(16);
    let lapis_z = origin_z + rng.next_i32_bound(16);
    let lapis_cfg = ore_config(stone, ids.lapis, 6);

    place_beta_ore(
        &lapis_cfg,
        BlockPos::new(lapis_x, lapis_y, lapis_z),
        volume,
        rng,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::generate::tests::corpus;
    use mcrs_minecraft_chunk::{Blocks, BoxVolume, VoxelId};
    use mcrs_minecraft_random::legacy::LegacyRandom;

    fn filled(state: BlockStateId) -> BoxVolume {
        BoxVolume::filled(
            BlockPos::new(-8, 40, -8),
            BlockPos::new(40, 100, 40),
            state.into(),
        )
    }

    #[test]
    fn clay_starts_only_in_water_and_turns_only_sand() {
        let ids = BetaOreBlockIds::resolve(corpus());
        let origin = BlockPos::new(8, 64, 8);
        let (sand, clay, water): (VoxelId, VoxelId, VoxelId) =
            (ids.sand.into(), ids.clay.into(), ids.water.into());

        let mut shore = filled(ids.sand);
        shore.set(origin, water);
        place_clay_vein(origin, &ids, &mut shore, &mut LegacyRandom::new(7));
        assert!(
            shore.iter().any(|(_, state)| state == clay),
            "no clay placed"
        );
        assert!(
            shore
                .iter()
                .all(|(_, state)| state == sand || state == clay || state == water),
            "the vein wrote something other than clay"
        );

        let mut rock = filled(ids.stone);
        rock.set(origin, water);
        place_clay_vein(origin, &ids, &mut rock, &mut LegacyRandom::new(7));
        assert!(
            rock.iter().all(|(_, state)| state != clay),
            "clay replaced stone"
        );

        let mut dry = filled(ids.sand);
        let untouched = LegacyRandom::new(7);
        let mut rng = untouched.clone();
        place_clay_vein(origin, &ids, &mut dry, &mut rng);
        assert!(
            dry.iter().all(|(_, state)| state == sand),
            "clay started out of water"
        );
        assert_eq!(rng, untouched, "a vein that does not start draws nothing");
    }
}
