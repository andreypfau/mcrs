use std::ops::Range;
use std::sync::Arc;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_chunk::{BlocksMut, VoxelId};
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

use super::placement::HeightmapName;
use crate::value_provider::HeightContext;

#[cfg(any(test, feature = "test-support"))]
mod box_region;
mod modifier;
mod predicate;
mod states;

#[cfg(any(test, feature = "test-support"))]
pub use box_region::BoxRegion;
pub use modifier::{Modifier, biome_info_noise};
pub use predicate::{Predicate, Rule, mask_of, single_state};
pub use states::{BlockLayout, PropertyLayout, WorldStates};

/// What a placement modifier or a generator may ask of the volume it runs in,
/// beyond the blocks themselves.
///
/// World-absolute coordinates throughout.
pub trait WorldGenVolume: BlocksMut {
    fn world(&self) -> &WorldStates;

    fn is_air(&self, p: BlockPos) -> bool {
        self.holds(&self.world().air_states, p)
    }

    fn holds(&self, mask: &FixedBitSet, p: BlockPos) -> bool {
        mask.contains(self.get(p).0 as usize)
    }

    /// `Feature.safeSetBlock`: the write happens unless the block there is in
    /// `unless`, and the answer is whether it did.
    fn set_unless(&mut self, unless: &FixedBitSet, p: BlockPos, state: VoxelId) -> bool {
        if self.holds(unless, p) {
            return false;
        }
        self.set(p, state);
        true
    }

    fn height(&self, kind: HeightmapName, x: i32, z: i32) -> i32;

    fn biome(&self, p: BlockPos) -> u32;

    fn extent(&self) -> HeightContext;

    /// `BlockState.canSurvive`, which is block behaviour rather than a property
    /// of the state, so only the volume can answer it.
    fn would_survive(&self, state: VoxelId, p: BlockPos) -> bool;
}

/// A set of block states as a bit per state id.
pub type StateMask = Arc<FixedBitSet>;

/// A set of biomes as a bit per the index [`WorldGenVolume::biome`] answers with.
pub type BiomeMask = Arc<FixedBitSet>;

/// The stack of positions with the modifier each is waiting on, plus the
/// output buffer, one set per worker. Cleared between objects, never
/// reallocated once warm.
#[derive(Debug, Default)]
pub struct PlacerScratch {
    pending: Vec<(BlockPos, usize)>,
    outputs: Vec<BlockPos>,
}

/// One object: run its modifier chain over `origin` and hand every surviving
/// position to `generate`.
///
/// The stack machine is the reference's, reverse push included — the random
/// source is shared by every modifier and by the generator, so the traversal
/// order is what fixes the draw order.
pub fn place<W: WorldGenVolume, R: Random>(
    modifiers: &[Modifier],
    volume: &mut W,
    scratch: &mut PlacerScratch,
    origin: BlockPos,
    rng: &mut R,
    carries: &dyn Fn(u32) -> bool,
    generate: &mut dyn FnMut(&mut W, &mut R, BlockPos) -> bool,
) -> bool {
    if modifiers.is_empty() {
        return generate(volume, rng, origin);
    }

    scratch.pending.clear();
    scratch.pending.push((origin, 0));

    let mut placed_any = false;
    while let Some((pos, index)) = scratch.pending.pop() {
        modifiers[index].apply(volume, rng, pos, carries, &mut scratch.outputs);
        let next = index + 1;
        if next < modifiers.len() {
            scratch
                .pending
                .extend(scratch.outputs.iter().rev().map(|&at| (at, next)));
        } else {
            for &at in &scratch.outputs {
                placed_any |= generate(volume, rng, at);
            }
        }
        scratch.outputs.clear();
    }
    placed_any
}

/// One object's generator: the object's `(step, index)`, the volume, the shared
/// source, the position, and whether a biome carries the object being placed —
/// which a feature holding a nested placed feature passes down unchanged.
pub type Generate<'a, W> = &'a mut dyn FnMut(
    (usize, usize),
    &mut W,
    &mut XoroshiroRandom,
    BlockPos,
    &dyn Fn(u32) -> bool,
) -> bool;

/// `steps` of a volume's decoration program: each step in order, every feature
/// of a step that the volume's biomes carry, ascending by global index, each on
/// its own source seeded from the column's decoration seed.
///
/// A feature's seed is a function of its own `(step, index)` alone, so a caller
/// that walks the program a few steps at a time draws exactly what one call
/// over the whole range would.
///
/// `present[step]` is the union over the volume's biomes; `carries` answers
/// whether one biome carries the feature at `(step, index)`, which is what the
/// `biome` filter tests.
#[allow(clippy::too_many_arguments)]
pub fn decorate<'a, W: WorldGenVolume>(
    present: &[FixedBitSet],
    steps: Range<usize>,
    chain: &dyn Fn(usize, usize) -> &'a [Modifier],
    carries: &dyn Fn(u32, usize, usize) -> bool,
    volume: &mut W,
    scratch: &mut PlacerScratch,
    origin: BlockPos,
    decoration_seed: i64,
    generate: Generate<'_, W>,
) {
    for step in steps {
        let Some(bits) = present.get(step) else {
            continue;
        };
        for index in bits.ones() {
            let seed = decoration_seed
                .wrapping_add(index as i64)
                .wrapping_add(10_000 * step as i64);
            let mut rng = XoroshiroRandom::new(seed as u64);
            let carry = |biome: u32| carries(biome, step, index);
            place(
                chain(step, index),
                volume,
                scratch,
                origin,
                &mut rng,
                &carry,
                &mut |volume, rng, at| generate((step, index), volume, rng, at, &carry),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::placement::VerticalDirection;
    use crate::feature::tree::{Bounded, UnitFloat};
    use crate::value_provider::{BoundedIntProvider, IntProvider};
    use bevy_math::IVec3;

    const AIR: VoxelId = VoxelId(0);
    const STONE: VoxelId = VoxelId(1);

    /// Stone to y=64, air above, one heightmap value, one biome.
    fn stub() -> BoxRegion {
        let mut volume = BoxRegion::columns(4, -64, 319, AIR)
            .floor(64, STONE)
            .with_height(|_, _, _, _| 65);
        volume.biome = 7;
        volume.world.air_states = mask_of([AIR.0]);
        volume
    }

    fn always(_: u32) -> bool {
        true
    }

    /// Runs a chain from `origin` and reports where the generator fired and what
    /// the shared source looked like afterwards.
    fn run(
        modifiers: &[Modifier],
        origin: BlockPos,
        seed: u64,
    ) -> (Vec<BlockPos>, XoroshiroRandom) {
        let mut volume = stub();
        let mut scratch = PlacerScratch::default();
        let mut rng = XoroshiroRandom::new(seed);
        let mut hits = Vec::new();
        place(
            modifiers,
            &mut volume,
            &mut scratch,
            origin,
            &mut rng,
            &always,
            &mut |_, _, at| {
                hits.push(at);
                true
            },
        );
        (hits, rng)
    }

    const ORIGIN: BlockPos = BlockPos::new(16, 0, 32);

    #[test]
    fn an_empty_chain_runs_the_generator_at_the_origin() {
        let (hits, rng) = run(&[], ORIGIN, 1);
        assert_eq!(hits, vec![ORIGIN]);
        assert_eq!(rng, XoroshiroRandom::new(1), "an empty chain draws nothing");
    }

    /// `count` emits three copies and `offset` consumes them one at a time. A
    /// placer that pushed its outputs in emission order would run the three
    /// offsets in the reverse order and place a different world, so pin both the
    /// draw sequence and where each copy landed.
    #[test]
    fn outputs_are_consumed_depth_first_in_emission_order() {
        let modifiers = vec![
            Modifier::Count {
                count: BoundedIntProvider(IntProvider::Constant(3)),
            },
            Modifier::Offset {
                x: BoundedIntProvider(IntProvider::uniform(0, 15)),
                y: BoundedIntProvider(IntProvider::Constant(0)),
                z: BoundedIntProvider(IntProvider::Constant(0)),
            },
        ];
        let (hits, rng) = run(&modifiers, ORIGIN, 42);

        let mut replay = XoroshiroRandom::new(42);
        let expected: Vec<BlockPos> = (0..3)
            .map(|_| {
                BlockPos::new(
                    ORIGIN.x + IntProvider::uniform(0, 15).sample(&mut replay),
                    0,
                    ORIGIN.z,
                )
            })
            .collect();
        assert_eq!(rng, replay, "three offset draws and nothing else");
        assert_eq!(hits, expected);
    }

    /// The whole first branch reaches the generator before the second branch's
    /// filter is rolled at all.
    #[test]
    fn a_filter_after_a_count_interleaves_one_branch_at_a_time() {
        let modifiers = vec![
            Modifier::Count {
                count: BoundedIntProvider(IntProvider::Constant(2)),
            },
            Modifier::RandomChance {
                chance: UnitFloat(0.5),
            },
            Modifier::Offset {
                x: BoundedIntProvider(IntProvider::uniform(0, 7)),
                y: BoundedIntProvider(IntProvider::Constant(0)),
                z: BoundedIntProvider(IntProvider::Constant(0)),
            },
        ];
        let (hits, rng) = run(&modifiers, ORIGIN, 9);

        let mut replay = XoroshiroRandom::new(9);
        let mut expected = Vec::new();
        for _ in 0..2 {
            if replay.next_f32() < 0.5 {
                let x = IntProvider::uniform(0, 7).sample(&mut replay);
                expected.push(BlockPos::new(ORIGIN.x + x, 0, ORIGIN.z));
            }
        }
        assert_eq!(rng, replay);
        assert_eq!(hits, expected);
    }

    #[test]
    fn in_square_draws_two_bounded_ints() {
        let (hits, rng) = run(&[Modifier::InSquare {}], ORIGIN, 7);
        let mut replay = XoroshiroRandom::new(7);
        let x = replay.next_i32_bound(16) + ORIGIN.x;
        let z = replay.next_i32_bound(16) + ORIGIN.z;
        assert_eq!(rng, replay);
        assert_eq!(hits, vec![BlockPos::new(x, 0, z)]);
    }

    #[test]
    fn heightmap_moves_to_the_map_and_draws_nothing() {
        let (hits, rng) = run(
            &[Modifier::Heightmap {
                heightmap: HeightmapName::WorldSurfaceWg,
            }],
            ORIGIN,
            3,
        );
        assert_eq!(rng, XoroshiroRandom::new(3));
        assert_eq!(hits, vec![BlockPos::new(16, 65, 32)]);
    }

    #[test]
    fn a_block_predicate_filter_draws_nothing() {
        let modifiers = vec![
            Modifier::Heightmap {
                heightmap: HeightmapName::WorldSurfaceWg,
            },
            Modifier::BlockPredicateFilter {
                predicate: Predicate::MatchingStates {
                    offset: IVec3::NEG_Y,
                    states: mask_of([STONE.0]),
                },
            },
        ];
        let (hits, rng) = run(&modifiers, ORIGIN, 5);
        assert_eq!(rng, XoroshiroRandom::new(5));
        assert_eq!(hits, vec![BlockPos::new(16, 65, 32)]);
    }

    #[test]
    fn cuboid_samples_the_height_before_the_two_widths() {
        let modifiers = vec![Modifier::Cuboid {
            xz_size: BoundedIntProvider(IntProvider::uniform(1, 3)),
            y_size: BoundedIntProvider(IntProvider::uniform(1, 3)),
            include_edges: true,
            include_interior: true,
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 11);

        let mut replay = XoroshiroRandom::new(11);
        let height = IntProvider::uniform(1, 3).sample(&mut replay);
        let width = IntProvider::uniform(1, 3).sample(&mut replay);
        let length = IntProvider::uniform(1, 3).sample(&mut replay);
        assert_eq!(rng, replay);
        assert_eq!(hits.len() as i32, (width + 1) * (height + 1) * (length + 1));
        assert_eq!(hits[0], ORIGIN);
        assert_eq!(
            hits[1],
            ORIGIN + IVec3::new(0, 0, 1),
            "z is the innermost axis"
        );
    }

    #[test]
    fn a_cuboid_shell_drops_the_interior() {
        let modifiers = vec![Modifier::Cuboid {
            xz_size: BoundedIntProvider(IntProvider::Constant(2)),
            y_size: BoundedIntProvider(IntProvider::Constant(2)),
            include_edges: true,
            include_interior: false,
        }];
        let (hits, _) = run(&modifiers, ORIGIN, 1);
        assert_eq!(hits.len(), 26, "a 3x3x3 without its centre");
    }

    #[test]
    fn randomly_selected_spends_one_draw_then_its_choice() {
        let modifiers = vec![Modifier::RandomlySelected {
            placements: vec![
                Modifier::InSquare {},
                Modifier::Offset {
                    x: BoundedIntProvider(IntProvider::Constant(1)),
                    y: BoundedIntProvider(IntProvider::Constant(2)),
                    z: BoundedIntProvider(IntProvider::Constant(3)),
                },
            ],
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 21);

        let mut replay = XoroshiroRandom::new(21);
        let expected = match replay.next_i32_bound(2) {
            0 => {
                let x = replay.next_i32_bound(16) + ORIGIN.x;
                let z = replay.next_i32_bound(16) + ORIGIN.z;
                BlockPos::new(x, 0, z)
            }
            _ => ORIGIN + IVec3::new(1, 2, 3),
        };
        assert_eq!(rng, replay);
        assert_eq!(hits, vec![expected]);
    }

    #[test]
    fn rarity_and_random_chance_each_spend_one_float() {
        for modifier in [
            Modifier::RarityFilter { chance: Bounded(4) },
            Modifier::RandomChance {
                chance: UnitFloat(0.25),
            },
        ] {
            let (_, rng) = run(&[modifier], ORIGIN, 1);
            let mut replay = XoroshiroRandom::new(1);
            replay.next_f32();
            assert_eq!(rng, replay);
        }
    }

    #[test]
    fn environment_scan_walks_to_the_target_and_draws_nothing() {
        let modifiers = vec![Modifier::EnvironmentScan {
            direction_of_search: VerticalDirection::Down,
            target_condition: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([STONE.0]),
            },
            allowed_search_condition: None,
            max_steps: Bounded(32),
        }];
        let (hits, rng) = run(&modifiers, BlockPos::new(0, 70, 0), 4);
        assert_eq!(rng, XoroshiroRandom::new(4));
        assert_eq!(hits, vec![BlockPos::new(0, 64, 0)]);
    }

    #[test]
    fn environment_scan_gives_up_once_the_search_condition_fails() {
        let modifiers = vec![Modifier::EnvironmentScan {
            direction_of_search: VerticalDirection::Down,
            target_condition: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: StateMask::default(),
            },
            allowed_search_condition: Some(Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([AIR.0]),
            }),
            max_steps: Bounded(32),
        }];
        let (hits, _) = run(&modifiers, BlockPos::new(0, 70, 0), 4);
        assert!(hits.is_empty());
    }

    #[test]
    fn the_biome_filter_asks_the_window_and_draws_nothing() {
        let mut volume = stub();
        let mut scratch = PlacerScratch::default();
        let mut rng = XoroshiroRandom::new(0);
        let mut hits = 0;
        let refuse = |biome: u32| biome != 7;
        place(
            &[Modifier::Biome {}],
            &mut volume,
            &mut scratch,
            BlockPos::new(0, 0, 0),
            &mut rng,
            &refuse,
            &mut |_, _, _| {
                hits += 1;
                true
            },
        );
        assert_eq!(hits, 0);
        assert_eq!(rng, XoroshiroRandom::new(0));
    }

    #[test]
    fn fixed_placement_keeps_only_its_own_chunk() {
        let modifiers = vec![Modifier::FixedPlacement {
            positions: vec![[20, 5, 35], [-3, 5, 35]],
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 6);
        assert_eq!(rng, XoroshiroRandom::new(6));
        assert_eq!(hits, vec![BlockPos::new(20, 5, 35)]);
    }

    #[test]
    fn surface_water_depth_reads_two_maps_and_draws_nothing() {
        let (hits, rng) = run(
            &[Modifier::SurfaceWaterDepthFilter { max_water_depth: 0 }],
            ORIGIN,
            8,
        );
        assert_eq!(rng, XoroshiroRandom::new(8));
        assert_eq!(hits, vec![ORIGIN], "the stub's two maps are equal");
    }

    /// The count provider is sampled once per loop test, so the pass that finds
    /// nothing still spends the sample that ends it.
    #[test]
    fn count_on_every_layer_descends_and_stops_when_a_pass_finds_nothing() {
        let modifiers = vec![Modifier::CountOnEveryLayer {
            count: BoundedIntProvider(IntProvider::Constant(1)),
        }];
        let (hits, rng) = run(&modifiers, ORIGIN, 12);

        let mut replay = XoroshiroRandom::new(12);
        let x = replay.next_i32_bound(16) + ORIGIN.x;
        let z = replay.next_i32_bound(16) + ORIGIN.z;
        replay.next_i32_bound(16);
        replay.next_i32_bound(16);
        assert_eq!(
            rng, replay,
            "one hit on layer 0, then a second pass that finds none"
        );
        assert_eq!(hits, vec![BlockPos::new(x, 65, z)]);
    }

    #[test]
    fn a_rule_test_draws_only_once_the_block_matched() {
        let rule = Rule::RandomStates {
            states: mask_of([STONE.0]),
            probability: 1.0,
        };
        let mut rng = XoroshiroRandom::new(2);
        assert!(!rule.test(AIR, 0, &mut rng));
        assert_eq!(rng, XoroshiroRandom::new(2), "no draw on a failed identity");
        assert!(rule.test(STONE, 0, &mut rng));
        let mut replay = XoroshiroRandom::new(2);
        replay.next_f32();
        assert_eq!(rng, replay);
    }

    #[test]
    fn the_scratch_is_drained_and_reused() {
        let modifiers = vec![
            Modifier::Count {
                count: BoundedIntProvider(IntProvider::Constant(4)),
            },
            Modifier::InSquare {},
        ];
        let mut volume = stub();
        let mut scratch = PlacerScratch::default();
        let mut rng = XoroshiroRandom::new(13);
        for _ in 0..8 {
            place(
                &modifiers,
                &mut volume,
                &mut scratch,
                BlockPos::new(0, 0, 0),
                &mut rng,
                &always,
                &mut |_, _, _| true,
            );
        }
        assert!(scratch.pending.is_empty());
        assert!(scratch.outputs.is_empty());
        assert!(scratch.pending.capacity() <= 8, "no unbounded growth");
    }

    /// Seed 2345 with no world seed in it, so this field is the same in every
    /// world. The two off-origin values are the reference's own noise, recorded
    /// so a change to the seed, the permutation stream or the sampler is caught;
    /// the origin is zero for any permutation table and pins the discarded
    /// lattice offset instead.
    #[test]
    fn the_count_noise_is_the_fixed_field() {
        assert_eq!(biome_info_noise(0.0, 0.0), 0.0);
        assert_eq!(
            biome_info_noise(100.0 / 200.0, 300.0 / 200.0),
            0.435_083_717_107_772_8
        );
        assert_eq!(biome_info_noise(-1.2, 3.7), 0.302_165_716_886_520_4);
    }

    #[test]
    fn the_step_loop_seeds_each_object_from_the_decoration_seed() {
        let in_square = || vec![Modifier::InSquare {}];
        let steps = vec![vec![in_square(), in_square()], vec![in_square()]];
        let mut present = vec![FixedBitSet::with_capacity(2), FixedBitSet::with_capacity(1)];
        present[0].insert(1);
        present[1].insert(0);

        let walk = |ranges: &[Range<usize>]| {
            let mut volume = stub();
            let mut scratch = PlacerScratch::default();
            let mut hits = Vec::new();
            for range in ranges {
                decorate(
                    &present,
                    range.clone(),
                    &|step, index| &steps[step][index],
                    &|_, _, _| true,
                    &mut volume,
                    &mut scratch,
                    ORIGIN,
                    777,
                    &mut |_, _, _, at, _| {
                        hits.push(at);
                        true
                    },
                );
            }
            hits
        };

        let expect = |seed: i64| {
            let mut rng = XoroshiroRandom::new(seed as u64);
            let x = rng.next_i32_bound(16) + ORIGIN.x;
            let z = rng.next_i32_bound(16) + ORIGIN.z;
            BlockPos::new(x, 0, z)
        };
        let whole = walk(std::slice::from_ref(&(0..2)));
        assert_eq!(
            whole,
            vec![expect(777 + 1), expect(777 + 10_000)],
            "seed is decoration_seed + index + 10000 * step, and a skipped index shifts nothing"
        );
        assert_eq!(
            walk(&[0..1, 1..2]),
            whole,
            "a seed names its own step, so cutting the walk into rungs draws the same objects"
        );
    }
}
