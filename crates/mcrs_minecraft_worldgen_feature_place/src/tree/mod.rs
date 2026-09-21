pub mod decorator;
pub mod foliage;
pub mod provider;
pub mod root;
pub mod size;
pub mod survive;
pub mod trunk;

use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::{Blocks, BlocksMut, BoxVolume, Volume, VoxelId};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::block_predicate::Direction;
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;
use mcrs_minecraft_worldgen_feature::tree::FeatureSize;

use self::decorator::{CompiledTreeDecorator, DecoratorContext, TreePalette, TreeSink};
use self::foliage::Foliage;
use self::provider::StateProvider;
use self::root::MangroveRoots;
use self::size::{min_clipped_height, size_at_height};
use self::survive::SurviveRule;
use self::trunk::{TreeContext, TreeStates, Trunk};

/// `LeavesBlock.getOptionalDistanceAt` and the transition that writes a new
/// `distance` back, both as tables over state ids.
///
/// A block in `prevents_nearby_leaf_decay` answers 0 and is never rewritten, so
/// its row may be the identity.
#[derive(Clone, Debug, Default)]
pub struct LeafDistances {
    entries: HashMap<u16, LeafEntry>,
}

#[derive(Clone, Copy, Debug)]
struct LeafEntry {
    distance: u8,
    by_distance: [VoxelId; 7],
}

impl LeafDistances {
    /// `by_distance[d - 1]` is the state with `distance = d`, for `d` in 1..=7.
    pub fn insert(&mut self, state: VoxelId, distance: u8, by_distance: [VoxelId; 7]) {
        self.entries.insert(
            state.0,
            LeafEntry {
                distance,
                by_distance,
            },
        );
    }

    pub fn distance_at(&self, state: VoxelId) -> Option<u8> {
        self.entries.get(&state.0).map(|entry| entry.distance)
    }

    pub fn with_distance(&self, state: VoxelId, distance: u8) -> VoxelId {
        match self.entries.get(&state.0) {
            Some(entry) if (1..=7).contains(&distance) => entry.by_distance[distance as usize - 1],
            _ => state,
        }
    }
}

/// The tables every tree of a dimension shares: what the world is made of, not
/// what one feature configures.
#[derive(Clone, Debug)]
pub struct TreeTables {
    pub states: TreeStates,
    pub palette: TreePalette,
    pub leaf_distance: LeafDistances,
    /// `BlockState.canSurvive` per block of a family the shapes cover, keyed by
    /// the block index of the state being tested.
    pub survive: HashMap<u32, SurviveRule>,
}

/// One `tree` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledTree {
    pub trunk_provider: StateProvider,
    pub foliage_provider: StateProvider,
    pub below_trunk_provider: StateProvider,
    pub trunk: Trunk,
    pub foliage: Foliage,
    pub roots: Option<MangroveRoots>,
    pub minimum_size: FeatureSize,
    pub decorators: Vec<CompiledTreeDecorator>,
    pub ignore_vines: bool,
    pub tables: Arc<TreeTables>,
}
/// `LeavesBlock.DECAY_DISTANCE`.
const MAX_LEAF_DISTANCE: usize = 7;

/// `TreeFeature.place`: the trunk and crown, then the decorators, then the leaf
/// relaxation over everything written.
pub fn place_tree<W: WorldGenVolume>(
    tree: &CompiledTree,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    sink: &mut dyn TreeSink<W>,
    origin: BlockPos,
) -> bool {
    let mut cx = TreeContext::new(
        volume,
        &tree.tables.states,
        &tree.trunk_provider,
        &tree.foliage_provider,
        &tree.below_trunk_provider,
    );
    let placed = do_place(tree, &mut cx, rng, origin);
    let logs = std::mem::take(&mut cx.logs);
    let leaves = std::mem::take(&mut cx.leaves);
    drop(cx);

    let Some(roots) = placed else {
        return false;
    };
    if logs.is_empty() && leaves.is_empty() {
        return false;
    }

    let mut decorations = Vec::new();
    if !tree.decorators.is_empty() {
        let mut context = DecoratorContext::new(volume, &tree.tables, sink, &logs, &leaves, &roots);
        for decorator in &tree.decorators {
            decorator.place(&mut context, rng);
        }
        decorations = context.into_decorations();
    }

    let mut written = roots.iter().chain(&logs).chain(&leaves).chain(&decorations);
    let Some(&first) = written.next() else {
        return false;
    };
    let (min, max) = written.fold((*first, *first), |(lo, hi), &pos| {
        (lo.min(*pos), hi.max(*pos))
    });
    update_leaves(
        tree,
        volume,
        BoxVolume::empty(min.into(), max.into()),
        &logs,
        &decorations,
        &roots,
    );
    true
}

/// The roots the tree wrote, or `None` when it refused to place at all.
fn do_place<W: WorldGenVolume>(
    tree: &CompiledTree,
    cx: &mut TreeContext<'_, W>,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
) -> Option<Vec<BlockPos>> {
    let tree_height = tree.trunk.tree_height(rng);
    let foliage_height = tree.foliage.foliage_height(rng, tree_height);
    let trunk_height = tree_height - foliage_height;
    let leaf_radius = tree.foliage.foliage_radius(rng, trunk_height);
    let trunk_origin = match &tree.roots {
        Some(roots) => roots.trunk_origin(rng, origin),
        None => origin,
    };

    let extent = cx.volume.extent();
    let world_max_y = extent.min_y + extent.depth - 1;
    let min_y = origin.y.min(trunk_origin.y);
    let max_y = origin.y.max(trunk_origin.y) + tree_height + 1;
    if min_y < extent.min_y + 1 || max_y > world_max_y + 1 {
        return None;
    }

    let free_height = max_free_tree_height(tree, cx, tree_height, trunk_origin);
    let accepted = free_height >= tree_height
        || min_clipped_height(&tree.minimum_size).is_some_and(|min| free_height >= min);
    if !accepted {
        return None;
    }

    let roots = match &tree.roots {
        Some(placer) => placer.place_roots(cx, rng, origin, trunk_origin)?,
        None => Vec::new(),
    };

    for attachment in tree.trunk.place_trunk(cx, rng, free_height, trunk_origin) {
        tree.foliage
            .create_foliage(cx, rng, attachment, foliage_height, leaf_radius);
    }
    Some(roots)
}

/// `TreeFeature.getMaxFreeTreeHeight`: the footprint of `minimum_size` scanned
/// bottom up, stopping two below the first obstruction. No draws.
fn max_free_tree_height<W: WorldGenVolume>(
    tree: &CompiledTree,
    cx: &TreeContext<'_, W>,
    max_tree_height: i32,
    tree_pos: BlockPos,
) -> i32 {
    for y in 0..=max_tree_height + 1 {
        let radius = size_at_height(&tree.minimum_size, max_tree_height, y);
        for x in -radius..=radius {
            for z in -radius..=radius {
                let at = tree_pos + IVec3::new(x, y, z);
                let vine = !tree.ignore_vines && cx.holds(&tree.tables.palette.vines, at);
                if !tree.trunk.is_free(cx, at) || vine {
                    return y - 2;
                }
            }
        }
    }
    max_tree_height
}

/// `TreeFeature.updateLeaves`: lower the `distance` of every leaf reachable
/// from a log inside the tree's bounding box.
///
/// The reference pops from a `HashSet`, so within one distance the order is
/// its own; this pops the most recently added, which is what makes the pass
/// reproducible here.
fn update_leaves<W: WorldGenVolume>(
    tree: &CompiledTree,
    volume: &mut W,
    mut shape: BoxVolume,
    logs: &[BlockPos],
    decorations: &[BlockPos],
    roots: &[BlockPos],
) {
    const IN_SHAPE: VoxelId = VoxelId(1);
    for &pos in decorations.iter().chain(roots) {
        shape.set(pos, IN_SHAPE);
    }

    let mut to_check: [Vec<BlockPos>; MAX_LEAF_DISTANCE] = Default::default();
    to_check[0].extend_from_slice(logs);

    let mut smallest = 0usize;
    loop {
        if smallest >= MAX_LEAF_DISTANCE {
            return;
        }
        let Some(pos) = to_check[smallest].pop() else {
            smallest += 1;
            continue;
        };
        if !shape.contains(pos) {
            continue;
        }
        if smallest != 0 {
            let state = volume.get(pos);
            let lowered = tree
                .tables
                .leaf_distance
                .with_distance(state, smallest as u8);
            if lowered != state {
                volume.set(pos, lowered);
            }
        }
        shape.set(pos, IN_SHAPE);

        for direction in Direction::all() {
            let neighbour = pos + direction.normal();
            if !shape.contains(neighbour) || shape.get(neighbour) == IN_SHAPE {
                continue;
            }
            let state = volume.get(neighbour);
            let Some(distance) = tree.tables.leaf_distance.distance_at(state) else {
                continue;
            };
            let lowered = distance.min(smallest as u8 + 1) as usize;
            if lowered < MAX_LEAF_DISTANCE {
                to_check[lowered].push(neighbour);
                smallest = smallest.min(lowered);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::decorator::EntitiesOnly;
    use mcrs_minecraft_chunk::{Blocks, BlocksMut};
    use mcrs_minecraft_core::codec::Bounded;
    use mcrs_minecraft_random::Random;
    use mcrs_minecraft_random::worldgen::WorldgenRandom;
    use mcrs_minecraft_worldgen_feature::placer::mask_of;
    use mcrs_minecraft_worldgen_feature::placer::{BoxRegion, WorldStates};
    use mcrs_minecraft_worldgen_feature::tree::TreeDecorator;
    use std::sync::Arc;

    const AIR: VoxelId = VoxelId(0);
    const DIRT: VoxelId = VoxelId(1);
    const LOG: VoxelId = VoxelId(2);
    const MUSHROOM: VoxelId = VoxelId(3);

    /// `leaves[distance]`, the block's seven states.
    const fn leaf(distance: u8) -> VoxelId {
        VoxelId(10 + distance as u16)
    }

    /// Dirt to y=63, air above, plus whatever a test overrides.
    fn flat() -> BoxRegion {
        let mut volume = BoxRegion::columns(2, -64, 319, AIR)
            .floor(63, DIRT)
            .with_height(|_, _, _, _| 64);
        volume.world = WorldStates {
            air_states: mask_of([AIR]),
            replaceable: mask_of([AIR]),
            solid_render: mask_of([DIRT, LOG]),
            ..WorldStates::default()
        };
        volume
    }

    fn blocked_at(mut volume: BoxRegion, at: BlockPos) -> BoxRegion {
        volume.blocks.set(at, DIRT);
        volume
    }

    fn empty() -> Arc<fixedbitset::FixedBitSet> {
        Arc::new(fixedbitset::FixedBitSet::with_capacity(32))
    }

    fn leaves() -> Vec<VoxelId> {
        (1..=7).map(leaf).collect()
    }

    fn states() -> TreeStates {
        let mut valid = leaves();
        valid.push(AIR);
        let mut air_or_leaves = leaves();
        air_or_leaves.push(AIR);
        TreeStates {
            valid_tree_pos: mask_of(valid),
            logs: mask_of([LOG]),
            air_or_leaves: mask_of(air_or_leaves),
            persistent: empty(),
            axis: HashMap::default(),
            waterlogged: HashMap::default(),
        }
    }

    fn palette() -> TreePalette {
        TreePalette {
            vines: empty(),
            shelf_mushrooms: empty(),
            vine_side: [AIR; 4],
            bee_nest: AIR,
            cocoa: [[AIR; 4]; 3],
            shelf_mushroom: [[AIR; 4]; 2],
            pale_hanging_moss: [AIR; 2],
            creaking_heart: AIR,
        }
    }

    fn distances() -> LeafDistances {
        let by_distance = [
            leaf(1),
            leaf(2),
            leaf(3),
            leaf(4),
            leaf(5),
            leaf(6),
            leaf(7),
        ];
        let mut table = LeafDistances::default();
        for distance in 1..=7u8 {
            table.insert(leaf(distance), distance, by_distance);
        }
        table
    }

    /// `straight` over a `blob` of radius 3 and no crown height: one row, whose
    /// corners the blob's skip predicate always refuses at `y == 0`.
    fn tree(min_clipped: Option<i32>, decorators: Vec<CompiledTreeDecorator>) -> CompiledTree {
        CompiledTree {
            trunk_provider: StateProvider::Simple(LOG),
            foliage_provider: StateProvider::Simple(leaf(7)),
            below_trunk_provider: StateProvider::Simple(DIRT),
            trunk: Trunk {
                placer: serde_json::from_str(
                    r#"{"type":"minecraft:straight_trunk_placer","base_height":5,"height_rand_a":2,"height_rand_b":0}"#,
                )
                .unwrap(),
                grow_through: None,
            },
            foliage: Foliage(
                serde_json::from_str(
                    r#"{"type":"minecraft:blob_foliage_placer","radius":3,"offset":0,"height":0}"#,
                )
                .unwrap(),
            ),
            roots: None,
            minimum_size: FeatureSize::TwoLayers {
                limit: Bounded(1),
                lower_size: Bounded(0),
                upper_size: Bounded(1),
                min_clipped_height: min_clipped.map(Bounded),
            },
            decorators,
            ignore_vines: false,
            tables: Arc::new(TreeTables {
                states: states(),
                palette: palette(),
                leaf_distance: distances(),
                survive: Default::default(),
            }),
        }
    }

    const ORIGIN: BlockPos = BlockPos::new(8, 64, 8);

    fn height_of(seed: u64) -> i32 {
        let mut replay = WorldgenRandom::new(seed);
        5 + replay.next_i32_bound(3) + replay.next_i32_bound(1)
    }

    #[test]
    fn a_tree_writes_its_trunk_and_crown_and_spends_the_draws_its_placers_ask_for() {
        let mut volume = flat();
        let mut rng = WorldgenRandom::new(1234);
        let mut entities = EntitiesOnly::default();
        assert!(place_tree(
            &tree(None, Vec::new()),
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));

        let mut replay = WorldgenRandom::new(1234);
        let height = 5 + replay.next_i32_bound(3) + replay.next_i32_bound(1);
        for _ in 0..4 {
            replay.next_i32_bound(2);
        }
        assert_eq!(
            rng, replay,
            "two height draws, and one per corner of the single crown row"
        );

        for y in 0..height {
            assert_eq!(volume.get(ORIGIN + IVec3::Y * y), LOG, "trunk at y={y}");
        }
        assert_eq!(volume.get(ORIGIN - IVec3::Y), DIRT);
        assert_eq!(volume.get(ORIGIN + IVec3::Y * height), leaf(1));
        assert!(entities.0.is_empty());
    }

    /// Distance 1 over the top log, then one per step out along the row: the
    /// leaves a tree writes carry the provider's 7 until this pass lowers them.
    #[test]
    fn the_leaf_pass_lowers_a_distance_per_step_from_the_trunk() {
        let mut volume = flat();
        let mut rng = WorldgenRandom::new(7);
        let mut entities = EntitiesOnly::default();
        assert!(place_tree(
            &tree(None, Vec::new()),
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));

        let crown = ORIGIN + IVec3::Y * height_of(7);
        assert_eq!(volume.get(crown), leaf(1));
        assert_eq!(volume.get(crown + IVec3::X), leaf(2));
        assert_eq!(volume.get(crown - IVec3::Z), leaf(2));
        assert_eq!(volume.get(crown + IVec3::X * 2), leaf(3));
        assert_eq!(volume.get(crown + IVec3::new(1, 0, 1)), leaf(3));
        assert_eq!(volume.get(crown + IVec3::new(2, 0, 1)), leaf(4));
    }

    /// The obstruction sits three above the origin, so the free height is one,
    /// and without a `min_clipped_height` the tree refuses to place at all.
    #[test]
    fn an_obstruction_clips_the_tree_and_a_short_clip_places_nothing() {
        let blocked = ORIGIN + IVec3::Y * 3;
        let mut entities = EntitiesOnly::default();

        let mut volume = blocked_at(flat(), blocked);
        let mut rng = WorldgenRandom::new(99);
        assert!(!place_tree(
            &tree(None, Vec::new()),
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));
        assert!(volume.writes.is_empty(), "a refused tree writes nothing");

        let mut volume = blocked_at(flat(), blocked);
        let mut rng = WorldgenRandom::new(99);
        assert!(place_tree(
            &tree(Some(1), Vec::new()),
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));
        assert_eq!(volume.get(ORIGIN), LOG);
        assert_eq!(
            volume.get(ORIGIN + IVec3::Y),
            leaf(1),
            "the crown sits on the clip"
        );
        assert_eq!(volume.get(blocked), DIRT, "the obstruction is untouched");
    }

    #[test]
    fn a_decorator_runs_after_the_crown_and_joins_the_bounding_box() {
        let proto: TreeDecorator = serde_json::from_str(
            r#"{"type":"minecraft:attached_to_logs","probability":1.0,"block_provider":{"type":"minecraft:simple_state_provider","state":"minecraft:x"},"directions":["north"]}"#,
        )
        .unwrap();
        let decorator = CompiledTreeDecorator(
            proto
                .map_provider(|_| Ok::<_, ()>(StateProvider::Simple(MUSHROOM)))
                .unwrap(),
        );
        let mut volume = flat();
        let mut rng = WorldgenRandom::new(5);
        let mut entities = EntitiesOnly::default();
        assert!(place_tree(
            &tree(None, vec![decorator]),
            &mut volume,
            &mut rng,
            &mut entities,
            ORIGIN
        ));

        let height = height_of(5);
        let attached = (0..height)
            .filter(|y| volume.get(ORIGIN + IVec3::new(0, *y, -1)) == MUSHROOM)
            .count();
        assert_eq!(attached, height as usize, "one per log, all of them north");
    }

    /// The relaxation takes the minimum of the distance it finds and the step
    /// it arrived at, so it lowers a 7 it walks past and leaves a 1 alone.
    #[test]
    fn the_pass_takes_the_minimum_of_what_it_finds_and_the_step() {
        for (existing, expected) in [(leaf(1), leaf(1)), (leaf(7), leaf(4))] {
            let crown = ORIGIN + IVec3::Y * height_of(3);
            let under_edge = crown + IVec3::new(2, -1, 0);
            let mut volume = flat();
            volume.blocks.set(under_edge, existing);

            let mut rng = WorldgenRandom::new(3);
            let mut entities = EntitiesOnly::default();
            assert!(place_tree(
                &tree(None, Vec::new()),
                &mut volume,
                &mut rng,
                &mut entities,
                ORIGIN
            ));
            assert_eq!(
                volume.get(crown + IVec3::X * 2),
                leaf(3),
                "the cell the walk reaches it from"
            );
            assert_eq!(volume.get(under_edge), expected);
        }
    }
}
