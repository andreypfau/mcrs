use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen::value_provider::IntProvider;

use std::sync::Arc;

use crate::feature::tree::TreeTables;
use crate::feature::tree::decorator::{CompiledTreeDecorator, DecoratorContext, TreeSink};
use crate::feature::tree::provider::StateProvider;
use crate::feature::tree::trunk::random_horizontal;

/// `FALLEN_LOG_MAX_FALL_HEIGHT_TO_GROUND` — the drop the log start is allowed
/// to search downward through, plus the one step up it begins from.
const MAX_FALL_HEIGHT: i32 = 6;
const MAX_GROUND_GAP: i32 = 2;

/// One `fallen_tree` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledFallenTree {
    pub trunk_provider: StateProvider,
    pub log_length: IntProvider,
    pub stump_decorators: Vec<CompiledTreeDecorator>,
    pub log_decorators: Vec<CompiledTreeDecorator>,
    pub tables: Arc<TreeTables>,
}

/// `FallenTreeFeature.place`, which always reports success: the stump goes in
/// unconditionally and only the log can be refused.
pub fn place_fallen_tree<W: WorldGenVolume>(
    tree: &CompiledFallenTree,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    sink: &mut dyn TreeSink<W>,
    origin: BlockPos,
) -> bool {
    let stump = place_log(tree, volume, rng, origin, None);
    decorate(tree, &tree.stump_decorators, volume, rng, sink, &[stump]);

    let direction = random_horizontal(rng);
    let log_length = tree.log_length.sample(rng) - 2;
    let mut start = origin + direction.normal() * (2 + rng.next_i32_bound(2));

    start.y += 1;
    for _ in 0..MAX_FALL_HEIGHT {
        if is_valid(tree, volume, start) && is_over_solid_ground(volume, start) {
            break;
        }
        start.y -= 1;
    }

    if !can_place_entire_log(tree, volume, log_length, start, direction) {
        return true;
    }

    let mut logs = Vec::with_capacity(log_length.max(0) as usize);
    let mut pos = start;
    for _ in 0..log_length {
        logs.push(place_log(tree, volume, rng, pos, Some(direction)));
        pos += direction.normal();
    }
    decorate(tree, &tree.log_decorators, volume, rng, sink, &logs);
    true
}

fn place_log<W: WorldGenVolume>(
    tree: &CompiledFallenTree,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    pos: BlockPos,
    sideways: Option<Direction>,
) -> BlockPos {
    let state = tree.trunk_provider.state(volume, rng, pos);
    let state = match sideways {
        Some(direction) => tree.tables.states.with_axis(state, direction.axis()),
        None => state,
    };
    volume.set(pos, state);
    pos
}

fn is_valid<W: WorldGenVolume>(tree: &CompiledFallenTree, volume: &W, pos: BlockPos) -> bool {
    volume.holds(&tree.tables.states.valid_tree_pos, pos)
}

/// `isFaceSturdy(UP)` as the full-collision-cube flag: a top slab or a farmland
/// block reads as no ground where the reference accepts it, and every surface
/// the corpus's fallen trees land on is a full cube.
fn is_over_solid_ground<W: WorldGenVolume>(volume: &W, pos: BlockPos) -> bool {
    volume.holds(&volume.world().sturdy_up, pos - IVec3::Y)
}

/// The whole log is refused unless every cell is free and no more than two in
/// a row hang over nothing.
fn can_place_entire_log<W: WorldGenVolume>(
    tree: &CompiledFallenTree,
    volume: &W,
    log_length: i32,
    start: BlockPos,
    direction: Direction,
) -> bool {
    let mut gap = 0;
    let mut pos = start;
    for _ in 0..log_length {
        if !is_valid(tree, volume, pos) {
            return false;
        }
        if is_over_solid_ground(volume, pos) {
            gap = 0;
        } else {
            gap += 1;
            if gap > MAX_GROUND_GAP {
                return false;
            }
        }
        pos += direction.normal();
    }
    true
}

/// A fallen tree hands its decorators logs and nothing else — no leaves, no
/// roots — and drops what they wrote: there is no leaf relaxation to seed.
fn decorate<W: WorldGenVolume>(
    tree: &CompiledFallenTree,
    decorators: &[CompiledTreeDecorator],
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    sink: &mut dyn TreeSink<W>,
    logs: &[BlockPos],
) {
    if decorators.is_empty() {
        return;
    }
    let mut context = DecoratorContext::new(volume, &tree.tables, sink, logs, &[], &[]);
    for decorator in decorators {
        decorator.place(&mut context, rng);
    }
}

#[cfg(test)]
mod tests {
    use crate::feature::tree::decorator::EntitiesOnly;
    use mcrs_minecraft_chunk::Blocks;
    use mcrs_minecraft_worldgen::feature::placer::mask_of;
    use std::sync::Arc;

    use mcrs_minecraft_chunk::VoxelId;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::value_provider::DispatchedIntProvider;

    use super::*;
    use crate::feature::tree::decorator::TreePalette;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};
    use crate::feature::tree::trunk::TreeStates;
    use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldStates};

    const DIRT: VoxelId = VoxelId(1);
    const LOG: VoxelId = VoxelId(2);

    /// Dirt at y = 63 under air, and a trunk provider that draws once per log
    /// so the anchor below sees the log count.
    fn fixture() -> (CompiledFallenTree, FakeVolume) {
        let mut states = TreeStates::default();
        states.valid_tree_pos = mask_of([AIR]);
        states.logs = mask_of([LOG]);
        states.air_or_leaves = mask_of([AIR]);
        states.persistent = StateMask::default();

        let tree = CompiledFallenTree {
            trunk_provider: StateProvider::Weighted(vec![(LOG, 1)]),
            log_length: IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                min_inclusive: 4,
                max_inclusive: 7,
            }),
            stump_decorators: Vec::new(),
            log_decorators: Vec::new(),
            tables: Arc::new(TreeTables {
                states,
                palette: palette(),
                leaf_distance: Default::default(),
                survive: Default::default(),
            }),
        };

        let mut volume = FakeVolume::default();
        volume.world = WorldStates {
            air_states: mask_of([AIR]),
            replaceable: mask_of([AIR]),
            solid_render: mask_of([DIRT, LOG]),
            sturdy_up: mask_of([DIRT]),
            ..WorldStates::default()
        };
        for x in -8..=8 {
            for z in -8..=8 {
                volume.blocks.insert((x, 63, z), DIRT);
            }
        }
        (tree, volume)
    }

    fn palette() -> TreePalette {
        TreePalette {
            vines: StateMask::default(),
            shelf_mushrooms: StateMask::default(),
            vine_side: [AIR; 4],
            bee_nest: AIR,
            cocoa: [[AIR; 4]; 3],
            shelf_mushroom: [[AIR; 4]; 2],
            pale_hanging_moss: [AIR; 2],
            creaking_heart: AIR,
        }
    }

    const ORIGIN: BlockPos = BlockPos::new(0, 64, 0);

    /// The draw order of `FallenTreeFeature.placeFallenTree`: the stump's own
    /// provider, then the fall direction, then the log's length, then the gap
    /// between stump and log, then one provider draw per log block.
    #[test]
    fn a_fallen_tree_lays_a_stump_and_a_log_and_spends_the_draws_in_that_order() {
        let (tree, mut volume) = fixture();
        let mut rng = XoroshiroRandom::new(1234);
        let mut entities = EntitiesOnly::default();
        assert!(place_fallen_tree(
            &tree,
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));

        let mut replay = XoroshiroRandom::new(1234);
        replay.next_i32_bound(1);
        let direction = Direction::HORIZONTAL[replay.next_i32_bound(4) as usize];
        let log_length = 4 + replay.next_i32_bound(4) - 2;
        let gap = 2 + replay.next_i32_bound(2);
        for _ in 0..log_length {
            replay.next_i32_bound(1);
        }
        assert_eq!(
            rng, replay,
            "one provider draw per log, and three of its own"
        );

        assert_eq!(volume.get(ORIGIN), LOG, "the stump");
        let start = ORIGIN + direction.normal() * gap;
        for step in 0..log_length {
            let at = start + direction.normal() * step;
            assert_eq!(volume.get(at), LOG, "the log at {step} of {log_length}");
            assert_eq!(at.y, 64, "the log rests on the ground it searched down to");
        }
        assert!(entities.0.is_empty());
    }

    /// `canPlaceEntireFallenLog` refuses a log that would hang over more than
    /// two cells of nothing, and the stump stays where it went in.
    #[test]
    fn a_log_over_a_hole_is_refused_and_the_stump_stays() {
        let (mut tree, mut volume) = fixture();
        tree.log_length = IntProvider::Constant(7);
        volume.blocks.clear();
        volume.blocks.insert((0, 63, 0), DIRT);

        let mut rng = XoroshiroRandom::new(1234);
        let mut entities = EntitiesOnly::default();
        assert!(place_fallen_tree(
            &tree,
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));
        assert_eq!(volume.get(ORIGIN), LOG);
        assert_eq!(
            volume.writes.len(),
            1,
            "only the stump: {:?}",
            volume.writes
        );
    }
}
