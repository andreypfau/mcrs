//! The modern ore vein against `OreOracle`, which runs the game's own
//! `OreFeature.place` over a world that is stone everywhere.

use bytes::Buf;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placer::{BoxRegion, Rule, WorldStates, single_state};
use mcrs_minecraft_worldgen_feature_place::ore_modern::{
    CompiledOre, OreReplacement, OreScratch, place_modern_ore,
};
use mcrs_minecraft_worldgen_testing::{dump_placements, dump_string};
use mcrs_voxel::BlockPos;
use std::collections::HashMap;

const MAGIC: &[u8; 8] = b"MCOREVN0";
pub(crate) const MIN_Y: i32 = -64;
const MAX_Y: i32 = 320;

struct DumpTarget {
    kind: String,
    block: String,
    probability: f32,
    state: String,
}

pub(crate) struct DumpCase {
    name: String,
    seed: i64,
    origin: [i32; 3],
    size: i32,
    discard_chance: f32,
    targets: Vec<DumpTarget>,
    result: bool,
    state_after: [i64; 2],
    placements: Vec<([i32; 3], String)>,
}

fn read_dump() -> Vec<DumpCase> {
    let source = fixture("ore_vein");
    let mut r = bytes::Bytes::from(source.dump(MAGIC));
    let case_count = r.get_u32_le() as usize;
    let cases: Vec<DumpCase> = (0..case_count)
        .map(|_| {
            let name = dump_string(&mut r);
            let seed = r.get_i64_le();
            let origin = [r.get_i32_le(), r.get_i32_le(), r.get_i32_le()];
            let size = r.get_i32_le();
            let discard_chance = r.get_f32_le();
            let target_count = r.get_u32_le() as usize;
            let targets = (0..target_count)
                .map(|_| DumpTarget {
                    kind: dump_string(&mut r),
                    block: dump_string(&mut r),
                    probability: r.get_f32_le(),
                    state: dump_string(&mut r),
                })
                .collect();
            let result = r.get_i32_le() != 0;
            let state_after = [r.get_i64_le(), r.get_i64_le()];
            let placements = dump_placements(&mut r);
            DumpCase {
                name,
                seed,
                origin,
                size,
                discard_chance,
                targets,
                result,
                state_after,
                placements,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", source.name());
    cases
}

/// State ids are handed out as the dump names states, so a name the dump never
/// mentions can never match a mask.
#[derive(Default)]
struct Interner {
    ids: HashMap<String, VoxelId>,
}

impl Interner {
    fn id(&mut self, name: &str) -> VoxelId {
        let next = VoxelId(self.ids.len() as u16);
        *self.ids.entry(name.to_string()).or_insert(next)
    }
}

/// A world whose one air state is `air`, and nothing else.
fn air_world(air: VoxelId) -> WorldStates {
    WorldStates {
        air,
        air_states: single_state(air),
        ..WorldStates::default()
    }
}

/// Stone everywhere within the build height around `origin` and air for a
/// few blocks past it, with `ocean_floor` standing in for
/// `level.getHeight(OCEAN_FLOOR_WG, …)`.
pub(crate) fn stone_world(
    origin: [i32; 3],
    stone: VoxelId,
    air: VoxelId,
    ocean_floor: i32,
) -> BoxRegion {
    let (x, z) = (origin[0], origin[2]);
    let mut world = BoxRegion::new(
        BlockPos::new(x - 48, MIN_Y - 8, z - 48),
        BlockPos::new(x + 48, MAX_Y + 7, z + 48),
        stone,
    )
    .with_height(move |_, _, _, _| ocean_floor);
    for y in (MIN_Y - 8..MIN_Y).chain(MAX_Y..MAX_Y + 8) {
        world.seed_layer(y, air);
    }
    world.extent.min_y = MIN_Y;
    world.extent.depth = MAX_Y - MIN_Y;
    world.world = air_world(air);
    world
}

mod exhaustive {
    use super::*;

    #[test]
    fn every_dumped_vein_matches_block_for_block() {
        let cases = read_dump();
        // One buffer across every case, the way a column's run holds it: the first
        // vein starts from an empty one and the rest would show any residue the
        // previous vein left behind.
        let mut scratch = OreScratch::default();
        for case in &cases {
            let mut interner = Interner::default();
            let stone = interner.id("minecraft:stone");
            let air = interner.id("minecraft:air");
            let targets = case
                .targets
                .iter()
                .map(|target| OreReplacement {
                    target: match target.kind.as_str() {
                        "always_true" => Rule::AlwaysTrue,
                        "block_match" => {
                            Rule::MatchingStates(single_state(interner.id(&target.block)))
                        }
                        "random_block_match" => Rule::RandomStates {
                            states: single_state(interner.id(&target.block)),
                            probability: target.probability,
                        },
                        other => panic!("{}: unhandled rule test {other}", case.name),
                    },
                    state: interner.id(&target.state),
                })
                .collect();
            let cfg = CompiledOre {
                targets,
                size: case.size,
                discard_chance_on_air_exposure: case.discard_chance,
            };
            let mut world = stone_world(case.origin, stone, air, MAX_Y);
            let mut rng = WorldgenRandom::new(case.seed as u64);
            let result = place_modern_ore(
                &cfg,
                &mut world,
                &mut rng,
                BlockPos::from(case.origin),
                &mut scratch,
            );

            assert_eq!(result, case.result, "{}: return value", case.name);
            let expected: Vec<(BlockPos, VoxelId)> = case
                .placements
                .iter()
                .map(|(pos, name)| (BlockPos::from(*pos), interner.id(name)))
                .collect();
            assert_eq!(world.writes, expected, "{}: writes in order", case.name);
            assert_eq!(
                [rng.next_java_long(), rng.next_java_long()],
                case.state_after,
                "{}: random state after the vein — the draw count diverged",
                case.name
            );
        }
    }
}
