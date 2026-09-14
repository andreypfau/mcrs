use std::sync::{Arc, LazyLock};

use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
use mcrs_minecraft_worldgen::feature::placer::{
    Predicate, StateMask, WorldGenVolume, biome_info_noise,
};
use mcrs_minecraft_worldgen::noise::simplex::SimplexNoise;
use mcrs_minecraft_worldgen::noise::stack::NoiseStack;
use mcrs_minecraft_worldgen::value_provider::IntProvider;
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;

use crate::feature::holds;
use crate::feature::speleothem::scan_column;
use crate::feature::tree::provider::StateProvider;
use crate::tables::BlockTables;

#[derive(Clone, Debug)]
pub struct CompiledDisk {
    pub state_provider: StateProvider,
    pub target: Predicate,
    pub radius: IntProvider,
    pub half_height: i32,
}

/// `DiskFeature`: one radius for the whole disc, then every column inside it
/// rewritten from the top down for as long as the target holds.
///
/// The provider runs once per accepted position, so a target that matches often
/// is also what a weighted provider draws against.
pub fn place_disk<W: WorldGenVolume>(
    cfg: &CompiledDisk,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let top = at.y + cfg.half_height;
    let bottom = at.y - cfg.half_height - 1;
    let radius = cfg.radius.sample(rng);

    let mut placed = false;
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dz * dz > radius * radius {
                continue;
            }
            for y in (bottom + 1..=top).rev() {
                let pos = BlockPos::new(at.x + dx, y, at.z + dz);
                if !cfg.target.test(volume, pos) {
                    continue;
                }
                if let Some(state) = cfg.state_provider.optional_state(volume, rng, pos) {
                    volume.set(pos, state);
                    placed = true;
                }
            }
        }
    }
    placed
}

#[derive(Clone, Debug)]
pub struct CompiledBlueIce {
    pub blue_ice: VoxelId,
    pub packed_ice: VoxelId,
    pub ice: VoxelId,
}

/// `BlueIceFeature`: a seed beside packed ice, then two hundred attempts to
/// grow outwards from what is already blue ice.
///
/// Every attempt spends two draws on its height and four more on the offset,
/// including the attempts whose position is rejected.
pub fn place_blue_ice<W: WorldGenVolume>(
    cfg: &CompiledBlueIce,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    if at.y > volume.extent().sea_level - 1 {
        return false;
    }
    if !volume.holds(&volume.world().water_states, at)
        && !volume.holds(&volume.world().water_states, at - IVec3::Y)
    {
        return false;
    }
    let beside_packed_ice = Direction::all()[1..]
        .iter()
        .any(|direction| volume.get(at + direction.normal()) == cfg.packed_ice);
    if !beside_packed_ice {
        return false;
    }

    volume.set(at, cfg.blue_ice);
    for _ in 0..200 {
        let y_offset = rng.next_i32_bound(5) - rng.next_i32_bound(6);
        let mut reach = 3;
        if y_offset < 2 {
            reach += y_offset / 2;
        }
        if reach < 1 {
            continue;
        }
        let pos = at
            + IVec3::new(
                rng.next_i32_bound(reach) - rng.next_i32_bound(reach),
                y_offset,
                rng.next_i32_bound(reach) - rng.next_i32_bound(reach),
            );
        let state = volume.get(pos);
        if !volume.is_air(pos)
            && !volume.holds(&volume.world().water_states, pos)
            && state != cfg.packed_ice
            && state != cfg.ice
        {
            continue;
        }
        for direction in Direction::all() {
            let neighbour = pos + direction.normal();
            if volume.get(neighbour) == cfg.blue_ice {
                volume.set(pos, cfg.blue_ice);
                break;
            }
        }
    }
    true
}

#[derive(Clone, Debug)]
pub struct CompiledUnderwaterMagma {
    pub floor_search_range: i32,
    pub placement_radius_around_floor: i32,
    pub placement_probability_per_valid_position: f32,
    pub magma: VoxelId,
}

/// `UnderwaterMagmaFeature`: find the floor of the water column, then roll for
/// every position of a cube around it.
///
/// The roll comes before the validity test, so the cube costs one float per
/// position whatever the world under it looks like.
pub fn place_underwater_magma<W: WorldGenVolume>(
    cfg: &CompiledUnderwaterMagma,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let Some(floor) = water_floor(cfg, volume, at) else {
        return false;
    };
    let radius = cfg.placement_radius_around_floor;
    let mut placed = false;
    // `BlockPos.betweenClosed`: x runs fastest, z slowest, and that is the order
    // the draws are spent in.
    for z in -radius..=radius {
        for y in -radius..=radius {
            for x in -radius..=radius {
                if rng.next_f32() >= cfg.placement_probability_per_valid_position {
                    continue;
                }
                let pos = BlockPos::new(at.x + x, floor + y, at.z + z);
                if is_valid_placement(volume, pos) {
                    volume.set(pos, cfg.magma);
                    placed = true;
                }
            }
        }
    }
    placed
}

fn water_floor<W: WorldGenVolume>(
    cfg: &CompiledUnderwaterMagma,
    volume: &W,
    at: BlockPos,
) -> Option<i32> {
    let water = &volume.world().water_states;
    scan_column(
        volume,
        at,
        cfg.floor_search_range,
        |s| holds(water, s),
        |s| !holds(water, s),
    )
    .and_then(|column| column.floor)
}

fn is_valid_placement<W: WorldGenVolume>(volume: &W, pos: BlockPos) -> bool {
    if volume.holds(&volume.world().water_states, pos) || volume.is_air(pos) {
        return false;
    }
    // `getFaceOcclusionShape` reduced to whole-cube occlusion: a slab under the
    // floor reads as see-through where the reference would not.
    if !volume.holds(&volume.world().solid_render, pos - IVec3::Y) {
        return false;
    }
    Direction::HORIZONTAL
        .iter()
        .all(|side| volume.holds(&volume.world().solid_render, pos + side.normal()))
}

/// The reference's `TEMPERATURE_NOISE`, `BIOME_INFO_NOISE` and
/// `FROZEN_TEMPERATURE_NOISE`: fixed seeds that no world seed enters, so every
/// world's snow line and ice patches follow the same three fields.
static TEMPERATURE_NOISE: LazyLock<SimplexNoise> =
    LazyLock::new(|| SimplexNoise::from_random_at_origin(&mut LegacyRandom::new(1234)));

static FROZEN_TEMPERATURE_NOISE: LazyLock<NoiseStack<SimplexNoise>> = LazyLock::new(|| {
    let mut random = LegacyRandom::new(3456);
    let mut builder = NoiseStack::builder();
    builder
        .add(
            SimplexNoise::from_random_at_origin(&mut random),
            1.0,
            0.14285715,
        )
        .add(
            SimplexNoise::from_random_at_origin(&mut random),
            0.5,
            0.2857143,
        )
        .add(
            SimplexNoise::from_random_at_origin(&mut random),
            0.25,
            0.5714286,
        );
    builder.build()
});

/// What a biome's climate settings say about freezing, per biome slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeClimate {
    pub base_temperature: f32,
    pub frozen: bool,
    pub has_precipitation: bool,
}

#[derive(Clone, Debug)]
pub struct CompiledFreezeTopLayer {
    /// Indexed by the slot [`WorldGenVolume::biome`] answers with.
    pub biomes: Vec<BiomeClimate>,
    pub ice: VoxelId,
    pub snow: VoxelId,
    /// A full stack of snow layers, which supports another layer above it.
    pub snow_layers_8: VoxelId,
    pub snow_states: StateMask,
    pub cannot_support_snow: StateMask,
    pub support_override_snow: StateMask,
    pub tables: Arc<BlockTables>,
}

/// `SnowAndFreezeFeature`: ice on the water and snow on the ground over the
/// whole column, wherever the biome is cold enough. It draws nothing.
pub fn place_freeze_top_layer<W: WorldGenVolume>(
    cfg: &CompiledFreezeTopLayer,
    volume: &mut W,
    at: BlockPos,
) -> bool {
    let sea_level = volume.extent().sea_level;
    for dx in 0..16 {
        for dz in 0..16 {
            let x = at.x + dx;
            let z = at.z + dz;
            let y = volume.height(HeightmapName::MotionBlocking, x, z);
            let top = BlockPos::new(x, y, z);
            let below = top - IVec3::Y;
            let Some(climate) = cfg.biomes.get(volume.biome(top) as usize) else {
                continue;
            };

            if should_freeze(climate, volume, below, sea_level) {
                volume.set(below, cfg.ice);
            }
            if should_snow(cfg, climate, volume, top, sea_level) {
                volume.set(top, cfg.snow);
                let under = volume.get(below);
                if let Some(snowy) = cfg.tables.snowy.get(&under) {
                    volume.set(below, *snowy);
                }
            }
        }
    }
    true
}

/// `Biome.shouldFreeze` with `checkNeighbors` false, which is the only caller
/// here and the reason the four horizontal water tests never run.
fn should_freeze<W: WorldGenVolume>(
    climate: &BiomeClimate,
    volume: &W,
    pos: BlockPos,
    sea_level: i32,
) -> bool {
    if warm_enough_to_rain(climate, pos, sea_level) || !volume.extent().contains(pos.y) {
        return false;
    }
    volume.holds(&volume.world().water_states, pos)
}

fn should_snow<W: WorldGenVolume>(
    cfg: &CompiledFreezeTopLayer,
    climate: &BiomeClimate,
    volume: &W,
    pos: BlockPos,
    sea_level: i32,
) -> bool {
    if !climate.has_precipitation || warm_enough_to_rain(climate, pos, sea_level) {
        return false;
    }
    if !volume.extent().contains(pos.y) {
        return false;
    }
    if !volume.is_air(pos) && !volume.holds(&cfg.snow_states, pos) {
        return false;
    }
    snow_survives(cfg, volume, pos - IVec3::Y)
}

/// `SnowLayerBlock.canSurvive`, given the block below.
fn snow_survives<W: WorldGenVolume>(
    cfg: &CompiledFreezeTopLayer,
    volume: &W,
    below: BlockPos,
) -> bool {
    if volume.holds(&cfg.cannot_support_snow, below) {
        return false;
    }
    if volume.holds(&cfg.support_override_snow, below) {
        return true;
    }
    volume.holds(&volume.world().sturdy_up, below) || volume.get(below) == cfg.snow_layers_8
}

/// Both tests are also gated on a block light below ten. Nothing has lit the
/// column while it decorates, so the gate is open at every position.
fn warm_enough_to_rain(climate: &BiomeClimate, pos: BlockPos, sea_level: i32) -> bool {
    temperature(climate, pos, sea_level) >= 0.15
}

fn temperature(climate: &BiomeClimate, pos: BlockPos, sea_level: i32) -> f32 {
    let adjusted = if climate.frozen {
        frozen_temperature(pos, climate.base_temperature)
    } else {
        climate.base_temperature
    };
    let snow_level = sea_level + 17;
    if pos.y <= snow_level {
        return adjusted;
    }
    let variation = TEMPERATURE_NOISE.sample_2d(
        (pos.x as f32 / 8.0) as f64,
        (pos.z as f32 / 8.0) as f64,
        1.0,
        1.0,
    ) as f32
        * 8.0;
    adjusted - (variation + pos.y as f32 - snow_level as f32) * 0.05 / 40.0
}

fn frozen_temperature(pos: BlockPos, base_temperature: f32) -> f32 {
    let large =
        (FROZEN_TEMPERATURE_NOISE.get(pos.x as f64 * 0.05, 0.0, pos.z as f64 * 0.05) * 7.0) as f64;
    let edge = biome_info_noise(pos.x as f64 * 0.2, pos.z as f64 * 0.2);
    if large + edge < 0.3 {
        let small = biome_info_noise(pos.x as f64 * 0.09, pos.z as f64 * 0.09);
        if small < 0.8 {
            return 0.2;
        }
    }
    base_temperature
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::value_provider::{DispatchedIntProvider, IntProvider};

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const DIRT: VoxelId = VoxelId(1);
    const CLAY: VoxelId = VoxelId(2);
    const WATER: VoxelId = VoxelId(3);
    const PACKED_ICE: VoxelId = VoxelId(4);
    const BLUE_ICE: VoxelId = VoxelId(5);
    const ICE: VoxelId = VoxelId(6);
    const STONE: VoxelId = VoxelId(7);
    const MAGMA: VoxelId = VoxelId(8);
    const SNOW: VoxelId = VoxelId(9);
    const GRASS: VoxelId = VoxelId(10);
    const SNOWY_GRASS: VoxelId = VoxelId(11);

    fn seeded() -> XoroshiroRandom {
        XoroshiroRandom::new(0x5ca1e)
    }

    /// Water is the one fluid; stone and dirt occlude; grass, stone and dirt
    /// hold snow up.
    fn shore(mut volume: FakeVolume) -> FakeVolume {
        volume.world.water_states = mask_of([WATER]);
        volume.world.solid_render = mask_of([STONE, DIRT]);
        volume.world.sturdy_up = mask_of([GRASS, STONE, DIRT]);
        volume
    }

    fn filled(state: VoxelId, top: i32) -> FakeVolume {
        let mut volume = shore(FakeVolume::default());
        for x in -12..=12 {
            for z in -12..=12 {
                for y in -8..=top {
                    volume.blocks.insert((x, y, z), state);
                }
            }
        }
        volume
    }

    /// One radius drawn for the whole disc, then the columns inside it are
    /// rewritten over the full height of the slab.
    #[test]
    fn a_disk_draws_one_radius_and_fills_a_flat_circle() {
        let cfg = CompiledDisk {
            state_provider: StateProvider::Simple(CLAY),
            target: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([DIRT]),
            },
            radius: IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                min_inclusive: 2,
                max_inclusive: 3,
            }),
            half_height: 1,
        };
        let mut volume = filled(DIRT, 40);
        let mut rng = seeded();
        assert!(place_disk(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 30, 0)
        ));

        let mut replay = seeded();
        let radius = cfg.radius.sample(&mut replay);
        assert_eq!(rng, replay, "a simple provider draws nothing per column");

        let expected: Vec<(i32, i32, i32)> = (-radius..=radius)
            .flat_map(|dz| (-radius..=radius).map(move |dx| (dx, dz)))
            .filter(|(dx, dz)| dx * dx + dz * dz <= radius * radius)
            .flat_map(|(dx, dz)| [(dx, 31, dz), (dx, 30, dz), (dx, 29, dz)])
            .collect();
        let mut written: Vec<(i32, i32, i32)> = volume.writes.iter().map(|(pos, _)| *pos).collect();
        let mut wanted = expected.clone();
        written.sort_unstable();
        wanted.sort_unstable();
        assert_eq!(
            written, wanted,
            "half_height 1 reaches one above the origin and two below"
        );
        assert!(volume.writes.iter().all(|(_, state)| *state == CLAY));
    }

    /// A target that matches nothing leaves the world alone, but the radius is
    /// drawn before the first block is read.
    #[test]
    fn a_disk_over_the_wrong_block_still_draws_its_radius() {
        let radius = IntProvider::Dispatched(DispatchedIntProvider::Uniform {
            min_inclusive: 2,
            max_inclusive: 3,
        });
        let cfg = CompiledDisk {
            state_provider: StateProvider::Simple(CLAY),
            target: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([WATER]),
            },
            radius: radius.clone(),
            half_height: 1,
        };
        let mut volume = filled(DIRT, 40);
        let mut rng = seeded();
        assert!(!place_disk(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 30, 0)
        ));
        assert!(volume.writes.is_empty());

        let mut replay = seeded();
        radius.sample(&mut replay);
        assert_eq!(rng, replay, "the radius is spent before the first read");
    }

    fn blue_ice_config() -> CompiledBlueIce {
        CompiledBlueIce {
            blue_ice: BLUE_ICE,
            packed_ice: PACKED_ICE,
            ice: ICE,
        }
    }

    /// Two hundred attempts, six draws each: two for the height and four for
    /// the horizontal offset, spent whether or not the position is taken.
    #[test]
    fn blue_ice_spends_six_draws_on_each_of_its_two_hundred_attempts() {
        let cfg = blue_ice_config();
        let mut volume = filled(WATER, 40);
        volume.blocks.insert((0, 31, 0), PACKED_ICE);
        let mut rng = seeded();
        assert!(place_blue_ice(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 30, 0)
        ));

        let mut replay = seeded();
        for _ in 0..200 {
            let y_offset = replay.next_i32_bound(5) - replay.next_i32_bound(6);
            let reach = if y_offset < 2 { 3 + y_offset / 2 } else { 3 };
            assert!(reach >= 1, "the reach never falls below one");
            replay.next_i32_bound(reach);
            replay.next_i32_bound(reach);
            replay.next_i32_bound(reach);
            replay.next_i32_bound(reach);
        }
        assert_eq!(rng, replay);

        assert_eq!(
            volume.writes.first().map(|(pos, _)| *pos),
            Some((0, 30, 0)),
            "the seed goes down before the attempts start"
        );
        assert!(volume.writes.iter().all(|(_, state)| *state == BLUE_ICE));
    }

    /// Above the sea line, out of the water, or with no packed ice beside it,
    /// the feature refuses before its first draw.
    #[test]
    fn blue_ice_refuses_without_spending_a_draw() {
        let cfg = blue_ice_config();
        let mut volume = filled(WATER, 40);
        volume.blocks.insert((0, 31, 0), PACKED_ICE);

        for origin in [
            BlockPos::new(0, 63, 0),
            BlockPos::new(0, 45, 0),
            BlockPos::new(4, 30, 4),
        ] {
            let mut rng = seeded();
            let before = rng.clone();
            let mut volume = shore(FakeVolume::with(volume.blocks.clone()));
            assert!(
                !place_blue_ice(&cfg, &mut volume, &mut rng, origin),
                "{origin} should be refused"
            );
            assert!(volume.writes.is_empty());
            assert_eq!(rng, before);
        }
    }

    fn magma_config(radius: i32) -> CompiledUnderwaterMagma {
        CompiledUnderwaterMagma {
            floor_search_range: 5,
            placement_radius_around_floor: radius,
            placement_probability_per_valid_position: 0.5,
            magma: MAGMA,
        }
    }

    /// One float per position of the cube around the floor, spent before the
    /// world is consulted at all.
    #[test]
    fn underwater_magma_rolls_once_for_every_position_of_its_cube() {
        let cfg = magma_config(1);
        let mut volume = filled(STONE, 30);
        for x in -12..=12 {
            for z in -12..=12 {
                for y in 31..=40 {
                    volume.blocks.insert((x, y, z), WATER);
                }
            }
        }
        let mut rng = seeded();
        place_underwater_magma(&cfg, &mut volume, &mut rng, BlockPos::new(0, 34, 0));

        let mut replay = seeded();
        for _ in 0..27 {
            replay.next_f32();
        }
        assert_eq!(rng, replay, "a radius of one is a cube of twenty-seven");

        assert!(volume.writes.iter().all(|(_, state)| *state == MAGMA));
        assert!(
            volume
                .writes
                .iter()
                .all(|((x, y, z), _)| { x.abs() <= 1 && z.abs() <= 1 && (29..=31).contains(y) }),
            "the cube is centred on the floor at y=30: {:?}",
            volume.writes
        );
    }

    /// A column that is not water at the origin has no floor, and no floor is a
    /// refusal before the first roll.
    #[test]
    fn underwater_magma_outside_water_draws_nothing() {
        let cfg = magma_config(1);
        let mut volume = filled(STONE, 40);
        let mut rng = seeded();
        let before = rng.clone();
        assert!(!place_underwater_magma(
            &cfg,
            &mut volume,
            &mut rng,
            BlockPos::new(0, 34, 0)
        ));
        assert!(volume.writes.is_empty());
        assert_eq!(rng, before);
    }

    fn freeze_config(climate: BiomeClimate) -> CompiledFreezeTopLayer {
        CompiledFreezeTopLayer {
            biomes: vec![climate],
            ice: ICE,
            snow: SNOW,
            snow_layers_8: VoxelId(15),
            snow_states: mask_of([SNOW]),
            cannot_support_snow: mask_of([ICE]),
            support_override_snow: StateMask::default(),
            tables: Arc::new(BlockTables {
                snowy: rustc_hash::FxHashMap::from_iter([(GRASS, SNOWY_GRASS)]),
                ..BlockTables::default()
            }),
        }
    }

    const COLD: BiomeClimate = BiomeClimate {
        base_temperature: 0.0,
        frozen: false,
        has_precipitation: true,
    };

    const WARM: BiomeClimate = BiomeClimate {
        base_temperature: 0.8,
        frozen: false,
        has_precipitation: true,
    };

    /// A surface of grass with one water hole: the hole freezes, the ground
    /// takes a layer of snow, and the block under it is marked snowy.
    fn frozen_surface(climate: BiomeClimate) -> (CompiledFreezeTopLayer, FakeVolume) {
        let mut volume = shore(FakeVolume::default());
        for x in 0..16 {
            for z in 0..16 {
                volume.blocks.insert((x, 62, z), GRASS);
                volume.heights.insert((x, z), 63);
            }
        }
        volume.blocks.insert((3, 62, 4), WATER);
        (freeze_config(climate), volume)
    }

    #[test]
    fn a_cold_column_freezes_its_water_and_snows_on_its_ground() {
        let (cfg, mut volume) = frozen_surface(COLD);
        assert!(place_freeze_top_layer(
            &cfg,
            &mut volume,
            BlockPos::new(0, 0, 0)
        ));

        assert_eq!(volume.blocks.get(&(3, 62, 4)), Some(&ICE));
        assert_eq!(
            volume.blocks.get(&(0, 63, 0)),
            Some(&SNOW),
            "the free position over the ground takes the layer"
        );
        assert_eq!(
            volume.blocks.get(&(0, 62, 0)),
            Some(&SNOWY_GRASS),
            "and the ground under it is marked"
        );
        assert!(
            volume.blocks.get(&(3, 63, 4)) != Some(&SNOW),
            "ice cannot support a snow layer"
        );
        assert!(
            volume
                .writes
                .iter()
                .all(|((x, _, z), _)| (0..16).contains(x) && (0..16).contains(z)),
            "the footprint is the column and nothing else"
        );
    }

    #[test]
    fn a_warm_column_is_left_alone() {
        let (cfg, mut volume) = frozen_surface(WARM);
        assert!(place_freeze_top_layer(
            &cfg,
            &mut volume,
            BlockPos::new(0, 0, 0)
        ));
        assert!(volume.writes.is_empty());
    }

    /// The height-adjusted temperature falls off above the snow line, so a
    /// column high enough freezes in a biome that is warm at sea level.
    #[test]
    fn height_cools_a_column_above_the_snow_line() {
        let climate = BiomeClimate {
            base_temperature: 0.16,
            frozen: false,
            has_precipitation: true,
        };
        assert!(!warm_enough_to_rain(&climate, BlockPos::new(0, 300, 0), 63));
        assert!(warm_enough_to_rain(&climate, BlockPos::new(0, 63, 0), 63));
    }
}
