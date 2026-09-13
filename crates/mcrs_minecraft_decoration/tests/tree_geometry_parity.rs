//! Tree geometry against `TreeOracle`, which runs the shipped `tree` features
//! over a `WorldGenLevel` whose floor is dirt to y 63 and air above.
//!
//! The dump holds every position the reference wrote, in first-write order,
//! with the state it ended up holding, plus two `nextLong` taken after `place`
//! returned. The order and the positions pin the geometry, the random state
//! pins the draw count.
//!
//! The one thing not compared is a leaf's `distance`. The reference's
//! relaxation pops from a `java.util.HashSet`, and the order it pops in decides
//! which of two competing distances is written last, so the value is the JVM's
//! and not the reference's. Everything else about a leaf state — the block,
//! `persistent`, `waterlogged` — is compared.

use bevy_math::IVec3;
use bytes::Buf;
use mcrs_minecraft_decoration::feature::tree::decorator::{
    CompiledTreeDecorator, EntitiesOnly, TreePalette,
};
use mcrs_minecraft_decoration::feature::tree::foliage::Foliage;
use mcrs_minecraft_decoration::feature::tree::provider::StateProvider;
use mcrs_minecraft_decoration::feature::tree::trunk::{TreeStates, Trunk};
use mcrs_minecraft_decoration::feature::tree::{
    CompiledTree, LeafDistances, TreeTables, place_tree,
};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::corpus::{dump_placements, dump_string, open_dump};
use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
use mcrs_minecraft_worldgen::feature::placer::{
    BoxRegion, Predicate, StateMask, WorldStates, mask_of,
};
use mcrs_minecraft_worldgen::feature::proto::Feature;
use mcrs_minecraft_worldgen::feature::tree::BlockStateProvider;
use mcrs_voxel_storage::Blocks as _;
use mcrs_voxel_storage::VoxelId;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

const MAGIC: &[u8; 8] = b"MCTREEG0";
const MIN_Y: i32 = -64;
const DEPTH: i32 = 384;
const FLOOR_TOP: i32 = 63;

struct DumpCase {
    feature: String,
    seed: i64,
    origin: [i32; 3],
    result: bool,
    state_after: [i64; 2],
    blocks: Vec<([i32; 3], String)>,
}

fn read_dump() -> Vec<DumpCase> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vanilla/tree_geometry.bin");
    let mut r = open_dump(&path, MAGIC);
    let case_count = r.get_u32_le() as usize;
    let cases: Vec<DumpCase> = (0..case_count)
        .map(|_| {
            let feature = dump_string(&mut r);
            let seed = r.get_i64_le();
            let origin = [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()];
            let result = r.get_i32_le() != 0;
            let state_after = [r.get_i64_le(), r.get_i64_le()];
            let blocks = dump_placements(&mut r);
            DumpCase {
                feature,
                seed,
                origin,
                result,
                state_after,
                blocks,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    cases
}

/// The block states this world can hold, named the way
/// `BlockStateParser.serialize` names them so a dumped palette entry resolves
/// by string. Only the blocks the covered features write are modelled; a state
/// the dump names and this table does not is a failure, not a skip.
struct Blocks {
    ids: HashMap<String, VoxelId>,
    names: Vec<String>,
    logs: Vec<VoxelId>,
    leaves: Vec<VoxelId>,
    air: VoxelId,
    dirt: VoxelId,
}

const LOG_BLOCKS: [&str; 8] = [
    "oak_log",
    "birch_log",
    "spruce_log",
    "jungle_log",
    "acacia_log",
    "dark_oak_log",
    "pale_oak_log",
    "cherry_log",
];

fn leaf_blocks() -> [String; 8] {
    LOG_BLOCKS.map(|log| log.replace("_log", "_leaves"))
}

/// `distance` runs 1..=7 and `RotatedPillarBlock.AXIS` x, y, z, in the order
/// `TreeStates` indexes them.
const AXES: [&str; 3] = ["x", "y", "z"];

impl Blocks {
    fn new() -> Self {
        let mut blocks = Blocks {
            ids: HashMap::new(),
            names: Vec::new(),
            logs: Vec::new(),
            leaves: Vec::new(),
            air: VoxelId(0),
            dirt: VoxelId(0),
        };
        blocks.air = blocks.intern("minecraft:air");
        blocks.dirt = blocks.intern("minecraft:dirt");
        blocks.intern("minecraft:bee_nest[facing=south,honey_level=0]");
        for block in LOG_BLOCKS {
            for axis in AXES {
                let id = blocks.intern(&format!("minecraft:{block}[axis={axis}]"));
                blocks.logs.push(id);
            }
        }
        for block in leaf_blocks() {
            for distance in 1..=7 {
                let id = blocks.intern(&format!(
                    "minecraft:{block}[distance={distance},persistent=false,waterlogged=false]"
                ));
                blocks.leaves.push(id);
            }
        }
        blocks
    }

    fn intern(&mut self, name: &str) -> VoxelId {
        if let Some(id) = self.ids.get(name) {
            return *id;
        }
        let id = VoxelId(self.names.len() as u16);
        self.names.push(name.to_string());
        self.ids.insert(name.to_string(), id);
        id
    }

    fn get(&self, name: &str) -> VoxelId {
        *self
            .ids
            .get(name)
            .unwrap_or_else(|| panic!("{name} is not modelled by this test's block table"))
    }

    fn states(&self) -> TreeStates {
        let mut axis = rustc_hash::FxHashMap::default();
        for by_axis in self.logs.chunks(3) {
            let by_axis: [VoxelId; 3] = by_axis.try_into().unwrap();
            for state in by_axis {
                axis.insert(state.0, by_axis);
            }
        }
        TreeStates {
            // Air plus `#replaceable_by_trees`, of which only `#leaves` occurs
            // in a world that is dirt, air and what the tree wrote.
            valid_tree_pos: mask_of(std::iter::once(self.air).chain(self.leaves.iter().copied())),
            logs: mask_of(self.logs.iter().copied()),
            air_or_leaves: mask_of(std::iter::once(self.air).chain(self.leaves.iter().copied())),
            persistent: StateMask::default(),
            axis,
            waterlogged: rustc_hash::FxHashMap::default(),
        }
    }

    /// What the flat world is made of: air is the one air, dirt and logs are
    /// the solid faces, and leaves are replaceable alongside air.
    fn world(&self) -> WorldStates {
        WorldStates {
            air: self.air,
            air_states: mask_of([self.air]),
            replaceable: mask_of(std::iter::once(self.air).chain(self.leaves.iter().copied())),
            solid_render: mask_of(std::iter::once(self.dirt).chain(self.logs.iter().copied())),
            ..WorldStates::default()
        }
    }

    fn palette(&self) -> TreePalette {
        TreePalette {
            vines: StateMask::default(),
            shelf_mushrooms: StateMask::default(),
            vine_side: [self.air; 4],
            bee_nest: self.get("minecraft:bee_nest[facing=south,honey_level=0]"),
            cocoa: [[self.air; 4]; 3],
            shelf_mushroom: [[self.air; 4]; 2],
            pale_hanging_moss: [self.air; 2],
            creaking_heart: self.air,
        }
    }

    fn leaf_distances(&self) -> LeafDistances {
        let mut distances = LeafDistances::default();
        for by_distance in self.leaves.chunks(7) {
            let by_distance: [VoxelId; 7] = by_distance.try_into().unwrap();
            for (index, state) in by_distance.iter().enumerate() {
                distances.insert(*state, index as u8 + 1, by_distance);
            }
        }
        distances
    }

    /// The `#cannot_replace_below_tree_trunk` states present in this world:
    /// `#dirt` covers the floor, and nothing else the tree writes is in it.
    fn below_trunk(&self) -> StateProvider {
        StateProvider::RuleBased {
            fallback: None,
            rules: vec![(
                Predicate::Not(Box::new(Predicate::MatchingStates {
                    offset: IVec3::ZERO,
                    states: mask_of([self.dirt]),
                })),
                StateProvider::Simple(self.dirt),
            )],
        }
    }
}

/// Dirt to `FLOOR_TOP` and air above, around `origin`. The height is the
/// topmost block that stops motion, leaves excepted for the no-leaves map.
fn flat_world(blocks: &Blocks, origin: IVec3) -> BoxRegion {
    let mut world = BoxRegion::new(
        IVec3::new(origin.x - 40, MIN_Y, origin.z - 40),
        IVec3::new(origin.x + 40, MIN_Y + DEPTH - 1, origin.z + 40),
        blocks.air,
    )
    .floor(FLOOR_TOP, blocks.dirt);
    world.extent.sea_level = FLOOR_TOP + 1;
    world.world = blocks.world();
    let (air, leaves) = (blocks.air, blocks.leaves.clone());
    world.with_height(move |volume, kind, x, z| {
        (MIN_Y..MIN_Y + DEPTH)
            .rev()
            .find(|&y| {
                let state = volume.get(IVec3::new(x, y, z));
                state != air
                    && !(leaves.contains(&state)
                        && matches!(kind, HeightmapName::MotionBlockingNoLeaves))
            })
            .map_or(MIN_Y, |y| y + 1)
    })
}

/// The features the dump is compared for; the rest of the dump is skipped. A
/// feature this test runs is read straight from `assets/minecraft/worldgen/feature`.
const COVERED: [&str; 20] = [
    "minecraft:oak",
    "minecraft:oak_bees_002",
    "minecraft:oak_bees_005",
    "minecraft:birch",
    "minecraft:birch_bees_0002",
    "minecraft:birch_bees_002",
    "minecraft:birch_bees_005",
    "minecraft:super_birch_bees_0002",
    "minecraft:jungle_tree_no_vine",
    "minecraft:jungle_bush",
    "minecraft:spruce",
    "minecraft:pine",
    "minecraft:acacia",
    "minecraft:dark_oak",
    "minecraft:pale_oak_bonemeal",
    "minecraft:fancy_oak",
    "minecraft:fancy_oak_bees_002",
    "minecraft:fancy_oak_bees_005",
    "minecraft:cherry",
    "minecraft:cherry_bees_005",
];

fn compiled(feature: &str, b: &Blocks, corpus: &[(String, Feature)]) -> Option<CompiledTree> {
    if !COVERED.contains(&feature) {
        return None;
    }
    let Some((_, Feature::Tree(config))) = corpus.iter().find(|(id, _)| id == feature) else {
        panic!("{feature} is not a tree of the corpus");
    };
    let simple = |provider: &BlockStateProvider| match provider {
        BlockStateProvider::Simple { state } => state.name.to_string(),
        other => panic!("{feature}: {other:?} is not a simple provider"),
    };
    Some(CompiledTree {
        trunk_provider: StateProvider::Simple(
            b.get(&format!("{}[axis=y]", simple(&config.trunk_provider))),
        ),
        foliage_provider: StateProvider::Simple(b.get(&format!(
            "{}[distance=7,persistent=false,waterlogged=false]",
            simple(&config.foliage_provider)
        ))),
        below_trunk_provider: b.below_trunk(),
        roots: None,
        trunk: Trunk {
            placer: config.trunk_placer.clone(),
            grow_through: None,
        },
        foliage: Foliage(config.foliage_placer.clone()),
        minimum_size: config.minimum_size.clone(),
        decorators: config
            .decorators
            .iter()
            .map(|decorator| {
                CompiledTreeDecorator(
                    decorator
                        .map_provider(|_| Err("the fixture trees carry no provider decorator"))
                        .unwrap(),
                )
            })
            .collect(),
        ignore_vines: config.ignore_vines,
        tables: Arc::new(TreeTables {
            states: b.states(),
            palette: b.palette(),
            leaf_distance: b.leaf_distances(),
            survive: Default::default(),
        }),
    })
}

/// A leaf's `distance` is the reference's `HashSet` iteration order talking, so
/// it is blinded before the comparison. The block and every other property stay.
fn blind_distance(writes: &[([i32; 3], String)]) -> Vec<([i32; 3], String)> {
    writes
        .iter()
        .map(|(pos, name)| {
            let Some(start) = name.find("distance=") else {
                return (*pos, name.clone());
            };
            let digits = start + "distance=".len();
            let end = name[digits..]
                .find(|c: char| !c.is_ascii_digit())
                .map_or(name.len(), |offset| digits + offset);
            (
                *pos,
                format!("{}distance=?{}", &name[..start], &name[end..]),
            )
        })
        .collect()
}

#[test]
fn every_covered_tree_matches_the_reference_block_for_block() {
    let cases = read_dump();
    assert_eq!(cases.len(), 180, "the dump lost cases");
    let blocks = Blocks::new();
    let corpus: Vec<(String, Feature)> = mcrs_minecraft_worldgen::corpus::registry("feature")
        .into_iter()
        .map(|(id, feature)| (id.to_string(), feature))
        .collect();

    let mut covered = std::collections::BTreeSet::new();
    let mut compared = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let Some(tree) = compiled(&case.feature, &blocks, &corpus) else {
            continue;
        };
        covered.insert(case.feature.clone());
        compared += 1;

        let origin = IVec3::from_array(case.origin);
        let mut world = flat_world(&blocks, origin);
        let mut entities = EntitiesOnly::default();
        let mut rng = XoroshiroRandom::new(case.seed as u64);
        let result = place_tree(&tree, &mut world, &mut rng, &mut entities, origin);

        let label = format!("{}@{}", case.feature, case.seed);
        if result != case.result {
            failures.push(format!(
                "{label}: returned {result}, reference {}",
                case.result
            ));
            continue;
        }

        let mut seen = HashSet::new();
        let got: Vec<([i32; 3], String)> = world
            .writes
            .iter()
            .filter(|(pos, _)| seen.insert(*pos))
            .map(|(pos, _)| {
                (
                    pos.to_array(),
                    blocks.names[world.get(*pos).0 as usize].clone(),
                )
            })
            .collect();

        let (got, want) = (blind_distance(&got), blind_distance(&case.blocks));
        if got != want {
            let index = got.iter().zip(want.iter()).position(|(a, b)| a != b);
            let after = [rng.next_i64(), rng.next_i64()];
            failures.push(format!(
                "{label}: {} writes against the reference's {}, draw count {} — first difference at {index:?}: got {:?} want {:?}",
                got.len(),
                want.len(),
                if after == case.state_after { "equal" } else { "different" },
                index.and_then(|i| got.get(i)),
                index.and_then(|i| want.get(i)),
            ));
            continue;
        }

        let after = [rng.next_i64(), rng.next_i64()];
        if after != case.state_after {
            failures.push(format!(
                "{label}: random state after the tree is {after:?}, reference {:?} — the draw count diverged",
                case.state_after
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {compared} compared cases diverge from the reference:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    assert_eq!(
        (covered.len(), compared),
        (20, 80),
        "the covered set shrank; a feature dropped out of the dump or out of the table"
    );
    println!("reference-verified: {covered:#?} over {compared} cases");
}
