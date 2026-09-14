use rustc_hash::FxHashMap as HashMap;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen::material::proto::CaveSurface;
use mcrs_minecraft_worldgen::value_provider::IntProvider;

use std::sync::Arc;

use crate::feature::holds;
use crate::feature::tree::decorator::java_set_order;
use crate::feature::tree::provider::StateProvider;
use crate::tables::BlockTables;

/// The `waterlogged = false` states and what each becomes with the property
/// set, as a table over state ids. A state outside it either lacks the property
/// or already carries it, which are the two cases the reference leaves alone.
#[derive(Clone, Debug, Default)]
pub struct Waterlogging {
    pub water: VoxelId,
    pub flooded: HashMap<u16, VoxelId>,
}

/// One `vegetation_patch` — or, with `waterlogged` set, one
/// `waterlogged_vegetation_patch` — with every name it carries resolved.
#[derive(Clone, Debug)]
pub struct CompiledVegetationPatch {
    pub replaceable: StateMask,
    pub ground_state: StateProvider,
    pub surface: CaveSurface,
    pub depth: IntProvider,
    pub extra_bottom_block_chance: f32,
    pub vertical_range: i32,
    pub vegetation_chance: f32,
    pub xz_radius: IntProvider,
    pub extra_edge_column_chance: f32,
    /// The `waterlogged_vegetation_patch` variant of the feature.
    pub waterlogged: bool,
    pub tables: Arc<BlockTables>,
}

/// `VegetationPatchFeature.place`. `place_vegetation` runs the nested placed
/// feature — its own modifier chain over the same random source.
pub fn place_vegetation_patch<W>(
    config: &CompiledVegetationPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
    place_vegetation: &mut dyn FnMut(&mut W, &mut XoroshiroRandom, BlockPos) -> bool,
) -> bool
where
    W: WorldGenVolume,
{
    let x_radius = config.xz_radius.sample(rng) + 1;
    let z_radius = config.xz_radius.sample(rng) + 1;
    let surface = place_ground_patch(config, volume, rng, origin, x_radius, z_radius);
    for position in &surface {
        if config.vegetation_chance > 0.0 && rng.next_f32() < config.vegetation_chance {
            grow(config, volume, rng, *position, place_vegetation);
        }
    }
    !surface.is_empty()
}

fn inwards(surface: CaveSurface) -> IVec3 {
    match surface {
        CaveSurface::Ceiling => IVec3::Y,
        CaveSurface::Floor => IVec3::NEG_Y,
    }
}

/// The ground columns the patch laid, in `java.util.HashSet` iteration order:
/// the reference collects them in a set and the vegetation pass draws once per
/// element as it walks it.
fn place_ground_patch<W>(
    config: &CompiledVegetationPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
    x_radius: i32,
    z_radius: i32,
) -> Vec<BlockPos>
where
    W: WorldGenVolume,
{
    let inwards = inwards(config.surface);
    let mut found = Vec::new();
    for dx in -x_radius..=x_radius {
        let x_edge = dx == -x_radius || dx == x_radius;
        for dz in -z_radius..=z_radius {
            let z_edge = dz == -z_radius || dz == z_radius;
            if x_edge && z_edge {
                continue;
            }
            if (x_edge || z_edge)
                && (config.extra_edge_column_chance == 0.0
                    || rng.next_f32() > config.extra_edge_column_chance)
            {
                continue;
            }

            let mut pos = origin + IVec3::new(dx, 0, dz);
            let mut steps = 0;
            while volume.is_air(pos) && steps < config.vertical_range {
                pos += inwards;
                steps += 1;
            }
            let mut steps = 0;
            while !volume.is_air(pos) && steps < config.vertical_range {
                pos -= inwards;
                steps += 1;
            }

            let below = pos + inwards;
            if !volume.is_air(pos) || !volume.holds(&volume.world().sturdy_up, below) {
                continue;
            }
            let mut depth = config.depth.sample(rng);
            if config.extra_bottom_block_chance > 0.0
                && rng.next_f32() < config.extra_bottom_block_chance
            {
                depth += 1;
            }
            if place_ground(config, volume, rng, below, depth) {
                found.push(below);
            }
        }
    }

    let surface = java_set_order(&found);
    if !config.waterlogged {
        return surface;
    }
    let waterlogging = &config.tables.waterlogging;
    let dry: Vec<BlockPos> = surface
        .into_iter()
        .filter(|pos| !exposed(volume, *pos))
        .collect();
    let flooded = java_set_order(&dry);
    for pos in &flooded {
        volume.set(*pos, waterlogging.water);
    }
    flooded
}

fn place_ground<W>(
    config: &CompiledVegetationPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    start: BlockPos,
    depth: i32,
) -> bool
where
    W: WorldGenVolume,
{
    let inwards = inwards(config.surface);
    let mut at = start;
    for placed in 0..depth {
        let to_place = config.ground_state.state(volume, rng, at);
        let current = volume.get(at);
        if volume.world().block_of(to_place) == volume.world().block_of(current) {
            continue;
        }
        if !holds(&config.replaceable, current) {
            return placed != 0;
        }
        volume.set(at, to_place);
        at += inwards;
    }
    true
}

/// `WaterloggedVegetationPatchFeature.isExposed`: a ground block with any of
/// these five faces unbacked would leak, so it stays ground rather than water.
const EXPOSED_SIDES: [IVec3; 5] = [
    IVec3::new(0, 0, -1),
    IVec3::new(1, 0, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, -1, 0),
];

fn exposed<W: WorldGenVolume>(volume: &W, pos: BlockPos) -> bool {
    EXPOSED_SIDES
        .iter()
        .any(|side| !volume.holds(&volume.world().sturdy_up, pos + *side))
}

fn grow<W>(
    config: &CompiledVegetationPatch,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    position: BlockPos,
    place_vegetation: &mut dyn FnMut(&mut W, &mut XoroshiroRandom, BlockPos) -> bool,
) where
    W: WorldGenVolume,
{
    let outwards = -inwards(config.surface);
    if !config.waterlogged {
        place_vegetation(volume, rng, position + outwards);
        return;
    }
    let waterlogging = &config.tables.waterlogging;
    if !place_vegetation(volume, rng, position - IVec3::Y + outwards) {
        return;
    }
    if let Some(&flooded) = waterlogging.flooded.get(&volume.get(position).0) {
        volume.set(position, flooded);
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_chunk::BlocksMut;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const MOSS: VoxelId = VoxelId(2);
    const WATER: VoxelId = VoxelId(3);
    const DRY: VoxelId = VoxelId(4);
    const WET: VoxelId = VoxelId(5);

    const FLOOR_TOP: i32 = 40;
    const ORIGIN: BlockPos = BlockPos::new(0, 44, 0);
    const SEED: u64 = 0x5eed_0f0f;

    fn config() -> CompiledVegetationPatch {
        CompiledVegetationPatch {
            replaceable: mask_of([STONE]),
            ground_state: StateProvider::Simple(MOSS),
            surface: CaveSurface::Floor,
            depth: IntProvider::Constant(1),
            extra_bottom_block_chance: 0.0,
            vertical_range: 5,
            vegetation_chance: 0.8,
            xz_radius: IntProvider::uniform(2, 3),
            extra_edge_column_chance: 0.0,
            waterlogged: false,
            tables: Arc::new(BlockTables::default()),
        }
    }

    /// Stone up to `FLOOR_TOP`, air above it, over a span wider than any patch
    /// this fixture places.
    fn flat_world() -> FakeVolume {
        let mut volume = FakeVolume::default();
        volume.world.sturdy_up = mask_of([STONE, MOSS, DRY, WET]);
        volume.world.block_of_state = (0..64u32).collect();
        for x in -12..=12 {
            for z in -12..=12 {
                for y in 0..=FLOOR_TOP {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        volume
    }

    fn radii(rng: &mut XoroshiroRandom, config: &CompiledVegetationPatch) -> (i32, i32) {
        (
            config.xz_radius.sample(rng) + 1,
            config.xz_radius.sample(rng) + 1,
        )
    }

    /// The two radius draws, one depth draw per column the patch accepts, and
    /// one vegetation draw per ground block it laid — in that order. With every
    /// edge column dropped by a zero edge chance, the accepted columns are the
    /// interior of the rectangle.
    #[test]
    fn a_floor_patch_lays_moss_over_the_interior_and_seeds_it() {
        let config = config();
        let mut volume = flat_world();
        let mut rng = XoroshiroRandom::new(SEED);
        let mut seeded: Vec<BlockPos> = Vec::new();

        assert!(place_vegetation_patch(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |_, _, at| {
                seeded.push(at);
                true
            }
        ));

        let mut replay = XoroshiroRandom::new(SEED);
        let (x_radius, z_radius) = radii(&mut replay, &config);
        let interior = ((2 * x_radius - 1) * (2 * z_radius - 1)) as usize;
        for _ in 0..interior {
            config.depth.sample(&mut replay);
        }
        let mut grown = 0;
        for _ in 0..interior {
            if replay.next_f32() < config.vegetation_chance {
                grown += 1;
            }
        }
        assert_eq!(
            rng, replay,
            "two radii, {interior} depths, {interior} rolls"
        );
        assert_eq!(seeded.len(), grown);

        let moss: Vec<_> = volume
            .writes
            .iter()
            .filter(|(_, state)| *state == MOSS)
            .collect();
        assert_eq!(moss.len(), interior, "one ground block per accepted column");
        assert!(
            moss.iter().all(|((_, y, _), _)| *y == FLOOR_TOP),
            "the patch sits on the floor's top layer: {moss:?}"
        );
        assert!(
            seeded
                .iter()
                .all(|at| at.y == FLOOR_TOP + 1 && at.x.abs() < x_radius && at.z.abs() < z_radius),
            "vegetation goes one block out from the ground it grows on: {seeded:?}"
        );
    }

    /// A non-zero edge chance spends one float on every edge column that is not
    /// a corner, before the column is scanned at all.
    #[test]
    fn the_edge_columns_each_cost_one_draw() {
        let mut config = config();
        config.extra_edge_column_chance = 0.35;
        config.vegetation_chance = 0.0;
        let mut volume = flat_world();
        let mut rng = XoroshiroRandom::new(SEED);
        assert!(place_vegetation_patch(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |_, _, _| true
        ));

        let mut replay = XoroshiroRandom::new(SEED);
        let (x_radius, z_radius) = radii(&mut replay, &config);
        let mut accepted = 0;
        for dx in -x_radius..=x_radius {
            let x_edge = dx == -x_radius || dx == x_radius;
            for dz in -z_radius..=z_radius {
                let z_edge = dz == -z_radius || dz == z_radius;
                if x_edge && z_edge {
                    continue;
                }
                if (x_edge || z_edge) && replay.next_f32() > config.extra_edge_column_chance {
                    continue;
                }
                accepted += 1;
                config.depth.sample(&mut replay);
            }
        }
        assert_eq!(rng, replay, "one roll per edge column, then a depth each");
        assert_eq!(
            volume
                .writes
                .iter()
                .filter(|(_, state)| *state == MOSS)
                .count(),
            accepted
        );
    }

    /// A column whose ground is not replaceable places nothing and contributes
    /// no vegetation roll, but still spends its depth draw.
    #[test]
    fn a_floor_the_patch_may_not_replace_stays_bare() {
        let mut config = config();
        config.replaceable = StateMask::default();
        let mut volume = flat_world();
        let mut rng = XoroshiroRandom::new(SEED);

        assert!(!place_vegetation_patch(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |_, _, _| panic!("nothing was seeded")
        ));

        let mut replay = XoroshiroRandom::new(SEED);
        let (x_radius, z_radius) = radii(&mut replay, &config);
        for _ in 0..(2 * x_radius - 1) * (2 * z_radius - 1) {
            config.depth.sample(&mut replay);
        }
        assert_eq!(rng, replay);
        assert!(volume.writes.is_empty());
    }

    /// Ground the patch would lay on top of its own block is left alone: the
    /// provider is still sampled, the write is not made, and the column does not
    /// descend, so a second patch over the first writes nothing.
    #[test]
    fn ground_that_is_already_the_patch_block_is_not_rewritten() {
        let config = config();
        let mut volume = flat_world();
        for x in -12..=12 {
            for z in -12..=12 {
                volume.blocks.insert((x, FLOOR_TOP, z), MOSS);
            }
        }
        let mut rng = XoroshiroRandom::new(SEED);
        assert!(place_vegetation_patch(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |_, _, _| true
        ));
        assert!(volume.writes.is_empty(), "{:?}", volume.writes);
    }

    /// The waterlogged variant turns every ground block whose five faces are
    /// backed into water, drops the exposed ones from the set, and sets the
    /// property on whatever the vegetation left in the water block.
    #[test]
    fn a_waterlogged_patch_floods_its_enclosed_ground() {
        let mut config = config();
        config.ground_state = StateProvider::Simple(DRY);
        config.replaceable = mask_of([STONE]);
        config.depth = IntProvider::Constant(2);
        config.waterlogged = true;
        config.tables = Arc::new(BlockTables {
            waterlogging: Waterlogging {
                water: WATER,
                flooded: HashMap::from_iter([(DRY.0, WET)]),
            },
            ..BlockTables::default()
        });

        let mut volume = flat_world();
        let mut rng = XoroshiroRandom::new(SEED);
        let mut seeded: Vec<BlockPos> = Vec::new();
        assert!(place_vegetation_patch(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |volume: &mut FakeVolume, _: &mut XoroshiroRandom, at: BlockPos| {
                seeded.push(at);
                volume.set(at, DRY);
                true
            }
        ));

        let water: Vec<_> = volume
            .writes
            .iter()
            .filter(|(_, state)| *state == WATER)
            .map(|((x, y, z), _)| BlockPos::new(*x, *y, *z))
            .collect();
        assert!(!water.is_empty());
        assert!(
            water.iter().all(|pos| pos.y == FLOOR_TOP),
            "the pool replaces the top ground layer: {water:?}"
        );
        assert!(
            seeded.iter().all(|at| water.contains(at)),
            "vegetation lands in the water blocks: {seeded:?}"
        );
        assert_eq!(
            volume
                .writes
                .iter()
                .filter(|(_, state)| *state == WET)
                .count(),
            seeded.len(),
            "each seeded block was waterlogged"
        );
    }

    /// A ceiling patch scans upwards and hangs its ground under the roof.
    #[test]
    fn a_ceiling_patch_climbs_to_the_roof() {
        let mut config = config();
        config.surface = CaveSurface::Ceiling;
        config.vegetation_chance = 0.0;

        let mut volume = flat_world();
        volume.blocks.clear();
        const ROOF: i32 = 50;
        for x in -12..=12 {
            for z in -12..=12 {
                for y in ROOF..=ROOF + 3 {
                    volume.blocks.insert((x, y, z), STONE);
                }
            }
        }
        let mut rng = XoroshiroRandom::new(SEED);
        assert!(place_vegetation_patch(
            &config,
            &mut volume,
            &mut rng,
            BlockPos::new(0, ROOF - 4, 0),
            &mut |_, _, _| true
        ));
        assert!(
            volume
                .writes
                .iter()
                .all(|((_, y, _), state)| *state == MOSS && *y == ROOF),
            "{:?}",
            volume.writes
        );
    }
}
