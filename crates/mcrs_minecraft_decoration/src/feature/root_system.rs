use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, StateMask, WorldGenVolume};

use crate::feature::tree::provider::StateProvider;
use crate::feature::tree::survive::SurviveRule;

/// `Direction.from2DDataValue`, which is the order the level test walks its
/// four corners in.
const BY_2D: [IVec3; 4] = [
    IVec3::new(0, 0, 1),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 0, -1),
    IVec3::new(1, 0, 0),
];

/// One `root_system` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledRootSystem {
    pub required_vertical_space_for_tree: i32,
    pub level_test_distance: i32,
    pub max_level_deviation: i32,
    pub root_radius: i32,
    pub root_replaceable: StateMask,
    pub root_state_provider: StateProvider,
    pub root_placement_attempts: i32,
    pub root_column_max_height: i32,
    pub hanging_root_radius: i32,
    pub hanging_roots_vertical_span: i32,
    pub hanging_root_state_provider: StateProvider,
    pub hanging_root_placement_attempts: i32,
    pub allowed_vertical_water_for_tree: i32,
    pub allowed_tree_position: Predicate,
    /// `canSurvive` of the state the hanging provider hands back, when its
    /// block is one of the families the freeze answers. A block outside them
    /// survives, which is the default `BlockBehaviour.canSurvive`.
    pub hanging_survive: Option<SurviveRule>,
}

/// `RootSystemFeature.place`. `place_tree` runs the nested placed feature — its
/// own modifier chain over the same random source.
pub fn place_root_system<W>(
    config: &CompiledRootSystem,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
    place_tree: &mut dyn FnMut(&mut W, &mut XoroshiroRandom, BlockPos) -> bool,
) -> bool
where
    W: WorldGenVolume,
{
    if !volume.is_air(origin) {
        return false;
    }
    if place_dirt_and_tree(config, volume, rng, origin, place_tree) {
        place_hanging_roots(config, volume, rng, origin);
    }
    true
}

fn place_dirt_and_tree<W>(
    config: &CompiledRootSystem,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
    place_tree: &mut dyn FnMut(&mut W, &mut XoroshiroRandom, BlockPos) -> bool,
) -> bool
where
    W: WorldGenVolume,
{
    let mut working = origin;
    for step in 0..config.root_column_max_height {
        working.y += 1;
        if volume.height(HeightmapName::WorldSurface, working.x, working.z) < working.y {
            return false;
        }
        if !config.allowed_tree_position.test(volume, working)
            || !space_for_tree(config, volume, working)
        {
            continue;
        }
        let below = working - IVec3::Y;
        if volume.holds(&volume.world().lava_fluid, below)
            || !volume.holds(&volume.world().solid, below)
        {
            return false;
        }
        if place_tree(volume, rng, working) {
            place_dirt(config, volume, rng, origin, origin.y + step);
            return true;
        }
    }
    false
}

/// `spaceForTree`: a clear run above, and — when the feature asks for it — four
/// corners whose ground sits within the allowed deviation.
fn space_for_tree<W: WorldGenVolume>(
    config: &CompiledRootSystem,
    volume: &W,
    pos: BlockPos,
) -> bool {
    let mut up = pos;
    for above_origin in 1..=config.required_vertical_space_for_tree {
        up.y += 1;
        let air = volume.is_air(up);
        let shallow_water = above_origin + 1 <= config.allowed_vertical_water_for_tree
            && volume.holds(&volume.world().water_fluid, up);
        if !air && !shallow_water {
            return false;
        }
    }

    if config.level_test_distance > 0 {
        for offset in BY_2D {
            let corner = pos + offset * config.level_test_distance;
            let deviation = IVec3::Y * config.max_level_deviation;
            if volume.is_air(corner - deviation) || !volume.is_air(corner + deviation) {
                return false;
            }
        }
    }
    true
}

fn place_dirt<W: WorldGenVolume>(
    config: &CompiledRootSystem,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
    target_height: i32,
) {
    for y in origin.y..target_height {
        for _ in 0..config.root_placement_attempts {
            let x = origin.x + rng.next_i32_bound(config.root_radius)
                - rng.next_i32_bound(config.root_radius);
            let z = origin.z + rng.next_i32_bound(config.root_radius)
                - rng.next_i32_bound(config.root_radius);
            let at = BlockPos::new(x, y, z);
            if volume.holds(&config.root_replaceable, at) {
                let state = config.root_state_provider.state(volume, rng, at);
                volume.set(at, state);
            }
        }
    }
}

fn place_hanging_roots<W: WorldGenVolume>(
    config: &CompiledRootSystem,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
) {
    for _ in 0..config.hanging_root_placement_attempts {
        let x = rng.next_i32_bound(config.hanging_root_radius)
            - rng.next_i32_bound(config.hanging_root_radius);
        let y = rng.next_i32_bound(config.hanging_roots_vertical_span)
            - rng.next_i32_bound(config.hanging_roots_vertical_span);
        let z = rng.next_i32_bound(config.hanging_root_radius)
            - rng.next_i32_bound(config.hanging_root_radius);
        let at = origin + IVec3::new(x, y, z);
        if !volume.is_air(at) {
            continue;
        }
        let state = config.hanging_root_state_provider.state(volume, rng, at);
        let survives = config
            .hanging_survive
            .as_ref()
            .is_none_or(|rule| rule.test(at, |q| volume.get(q)));
        if survives && volume.holds(&volume.world().solid, at + IVec3::Y) {
            volume.set(at, state);
        }
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_voxel_storage::VoxelId;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const DIRT: VoxelId = VoxelId(1);
    const ROOTED: VoxelId = VoxelId(2);
    const HANGING: VoxelId = VoxelId(3);

    const ORIGIN: BlockPos = BlockPos::new(0, 30, 0);
    const SURFACE: i32 = 63;

    /// A dirt column with one air cell at the origin, air above the surface,
    /// and a `WORLD_SURFACE` that never stops the climb.
    fn fixture() -> (CompiledRootSystem, FakeVolume) {
        let config = CompiledRootSystem {
            required_vertical_space_for_tree: 3,
            level_test_distance: 0,
            max_level_deviation: 0,
            root_radius: 3,
            root_replaceable: mask_of([DIRT]),
            root_state_provider: StateProvider::Simple(ROOTED),
            root_placement_attempts: 2,
            root_column_max_height: 100,
            hanging_root_radius: 2,
            hanging_roots_vertical_span: 2,
            hanging_root_state_provider: StateProvider::Simple(HANGING),
            hanging_root_placement_attempts: 2,
            allowed_vertical_water_for_tree: 1,
            allowed_tree_position: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([AIR]),
            },
            hanging_survive: None,
        };

        let mut volume = FakeVolume::default();
        volume.world.solid = mask_of([DIRT]);
        for x in -8..=8 {
            for z in -8..=8 {
                for y in 0..=SURFACE {
                    volume.blocks.insert((x, y, z), DIRT);
                }
                volume.heights.insert((x, z), 200);
            }
        }
        volume.blocks.insert((ORIGIN.x, ORIGIN.y, ORIGIN.z), AIR);
        (config, volume)
    }

    /// The climb stops at the first cell the predicate accepts with solid
    /// ground under it, the dirt column runs from the origin up to it, and the
    /// draws are the reference's: four per rooted-dirt attempt, six per
    /// hanging-root attempt.
    #[test]
    fn a_root_system_plants_its_tree_and_fills_the_column_under_it() {
        let (config, mut volume) = fixture();
        let mut rng = XoroshiroRandom::new(4242);
        let mut planted = None;
        assert!(place_root_system(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |_, _, at| {
                planted = Some(at);
                true
            }
        ));
        assert_eq!(
            planted,
            Some(BlockPos::new(0, SURFACE + 1, 0)),
            "the first air cell over solid ground"
        );

        let rows = SURFACE - ORIGIN.y;
        let mut replay = XoroshiroRandom::new(4242);
        for _ in 0..rows * config.root_placement_attempts {
            for _ in 0..4 {
                replay.next_i32_bound(config.root_radius);
            }
        }
        for _ in 0..config.hanging_root_placement_attempts {
            replay.next_i32_bound(config.hanging_root_radius);
            replay.next_i32_bound(config.hanging_root_radius);
            replay.next_i32_bound(config.hanging_roots_vertical_span);
            replay.next_i32_bound(config.hanging_roots_vertical_span);
            replay.next_i32_bound(config.hanging_root_radius);
            replay.next_i32_bound(config.hanging_root_radius);
        }
        assert_eq!(
            rng, replay,
            "four draws per dirt attempt over {rows} rows, then six per hanging attempt"
        );

        let rooted = volume
            .writes
            .iter()
            .filter(|(_, state)| *state == ROOTED)
            .count();
        assert!(rooted > 0, "the column under the tree is rooted");
        assert!(
            volume
                .writes
                .iter()
                .all(|((_, y, _), state)| *state != ROOTED || (ORIGIN.y..SURFACE).contains(y)),
            "rooted dirt stays between the origin and the tree: {:?}",
            volume.writes
        );
    }

    /// The whole feature is a no-op when the origin is not air, and it reports
    /// failure rather than the unconditional success of the placed case.
    #[test]
    fn an_origin_that_is_not_air_places_nothing() {
        let (config, mut volume) = fixture();
        volume.blocks.insert((ORIGIN.x, ORIGIN.y, ORIGIN.z), DIRT);
        let mut rng = XoroshiroRandom::new(4242);
        let before = rng.clone();
        assert!(!place_root_system(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |_, _, _| panic!("no tree is planted")
        ));
        assert!(volume.writes.is_empty());
        assert_eq!(rng, before, "and it draws nothing");
    }

    /// A tree the nested feature refuses does not stop the climb, and with no
    /// spot accepted nothing is rooted — but the call still reports success.
    #[test]
    fn a_refused_tree_leaves_the_column_untouched() {
        let (config, mut volume) = fixture();
        let mut rng = XoroshiroRandom::new(4242);
        let mut attempts = 0;
        assert!(place_root_system(
            &config,
            &mut volume,
            &mut rng,
            ORIGIN,
            &mut |_, _, _| {
                attempts += 1;
                false
            }
        ));
        assert!(attempts > 0, "the nested feature was offered a position");
        assert!(volume.writes.is_empty(), "{:?}", volume.writes);
    }
}
