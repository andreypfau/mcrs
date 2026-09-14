use crate::feature::random_direction;
use rustc_hash::FxHashMap as HashMap;
use std::sync::Arc;

use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_core::mth::clamped_map;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{BlockLayout, Predicate, WorldGenVolume};
use mcrs_minecraft_worldgen::noise::stack::{NoiseStack, Octave};
use mcrs_minecraft_worldgen::value_provider::{IntProvider, pick_weighted_by};

pub type SharedNoise = Arc<NoiseStack<Octave>>;

/// A `BlockStateProvider` with every block, state and tag it names resolved.
///
/// The noise members hold the sampler the seed already built; they draw nothing.
#[derive(Clone, Debug)]
pub enum StateProvider {
    Simple(VoxelId),
    Weighted(Vec<(VoxelId, i32)>),
    RuleBased {
        fallback: Option<Box<StateProvider>>,
        rules: Vec<(Predicate, StateProvider)>,
    },
    RandomizedInt {
        source: Box<StateProvider>,
        /// The named property of every block that declares it as an integer,
        /// by block index; a block absent here leaves the state alone.
        property: Arc<HashMap<u32, IntProperty>>,
        values: IntProvider,
    },
    Rotated {
        source: Box<StateProvider>,
        direction: Option<Direction>,
        /// What `axis`, `facing` and `horizontal_facing` become for each
        /// direction, per block that declares any of them.
        rotations: Arc<HashMap<u32, Rotations>>,
    },
    RandomBlock(Vec<VoxelId>),
    CopyProperties(Box<StateProvider>),
    Noise {
        noise: SharedNoise,
        scale: f32,
        states: Vec<VoxelId>,
    },
    NoiseThreshold {
        noise: SharedNoise,
        scale: f32,
        threshold: f32,
        high_chance: f32,
        default_state: VoxelId,
        low_states: Vec<VoxelId>,
        high_states: Vec<VoxelId>,
    },
    DualNoise {
        slow_noise: SharedNoise,
        slow_scale: f32,
        variety_min: i32,
        variety_max: i32,
        noise: SharedNoise,
        scale: f32,
        states: Vec<VoxelId>,
    },
}

/// One integer property of one block as a digit of its state ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntProperty {
    pub base: u16,
    pub stride: u16,
    pub count: u16,
    pub min: i32,
}

impl IntProperty {
    /// `BlockState.setValue`, except that a value outside the property leaves
    /// the state alone rather than throwing.
    pub fn set(&self, state: VoxelId, value: i32) -> VoxelId {
        let index = value - self.min;
        if !(0..self.count as i32).contains(&index) {
            return state;
        }
        let current = ((state.0 - self.base) / self.stride % self.count) as i32;
        VoxelId((state.0 as i32 + (index - current) * self.stride as i32) as u16)
    }
}

/// The value index each direction selects in one property, or `None` where the
/// property lacks that direction (`horizontal_facing` has no up or down).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectionProperty {
    pub base: u16,
    pub stride: u16,
    pub count: u16,
    pub by_direction: [Option<u16>; 6],
}

impl DirectionProperty {
    fn set(&self, state: VoxelId, direction: Direction) -> VoxelId {
        let Some(index) = self.by_direction[direction as usize] else {
            return state;
        };
        let current = (state.0 - self.base) / self.stride % self.count;
        VoxelId((state.0 as i32 + (index as i32 - current as i32) * self.stride as i32) as u16)
    }
}

/// `RotatedBlockProvider`'s three `trySetValue`s over one block: `axis` to the
/// direction's axis, `facing` to the direction, and `horizontal_facing` to it
/// unless it is vertical.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rotations {
    pub axis: Option<DirectionProperty>,
    pub facing: Option<DirectionProperty>,
    pub horizontal_facing: Option<DirectionProperty>,
}

impl Rotations {
    pub fn rotate(&self, state: VoxelId, direction: Direction) -> VoxelId {
        let mut state = state;
        if let Some(axis) = &self.axis {
            state = axis.set(state, direction);
        }
        if let Some(facing) = &self.facing {
            state = facing.set(state, direction);
        }
        if let Some(horizontal) = &self.horizontal_facing
            && !direction.is_vertical()
        {
            state = horizontal.set(state, direction);
        }
        state
    }
}

/// [`Rotations`] for every block that declares any of the three properties,
/// resolved once so the provider never looks a property up by name while
/// placing.
pub fn rotation_table(layouts: &[BlockLayout]) -> HashMap<u32, Rotations> {
    let direction_property =
        |layout: &BlockLayout, name: &str, text: fn(Direction) -> &'static str| {
            let property = layout.property(name)?;
            Some(DirectionProperty {
                base: layout.base,
                stride: property.stride,
                count: property.values.len() as u16,
                by_direction: Direction::all().map(|direction| {
                    property
                        .values
                        .iter()
                        .position(|value| &**value == text(direction))
                        .map(|index| index as u16)
                }),
            })
        };
    layouts
        .iter()
        .enumerate()
        .filter_map(|(block, layout)| {
            let rotations = Rotations {
                axis: direction_property(layout, "axis", |direction| direction.axis().name()),
                facing: direction_property(layout, "facing", |direction| direction.name()),
                horizontal_facing: direction_property(layout, "horizontal_facing", |direction| {
                    direction.name()
                }),
            };
            (rotations != Rotations::default()).then_some((block as u32, rotations))
        })
        .collect()
}

/// `IntegerProperty` `name` on every block that declares it, resolved once so
/// the provider never looks a property up by name while placing.
pub fn int_property_table(layouts: &[BlockLayout], name: &str) -> HashMap<u32, IntProperty> {
    layouts
        .iter()
        .enumerate()
        .filter_map(|(block, layout)| {
            let property = layout.property(name)?;
            let ints: Vec<i32> = property
                .values
                .iter()
                .map(|value| value.parse().ok())
                .collect::<Option<_>>()?;
            let min = *ints.first()?;
            (ints.iter().enumerate().all(|(i, v)| *v == min + i as i32)).then_some((
                block as u32,
                IntProperty {
                    base: layout.base,
                    stride: property.stride,
                    count: ints.len() as u16,
                    min,
                },
            ))
        })
        .collect()
}

impl StateProvider {
    pub fn state<W: WorldGenVolume>(
        &self,
        volume: &W,
        rng: &mut XoroshiroRandom,
        pos: BlockPos,
    ) -> VoxelId {
        self.optional_state(volume, rng, pos)
            .unwrap_or_else(|| volume.get(pos))
    }

    pub fn optional_state<W: WorldGenVolume>(
        &self,
        volume: &W,
        rng: &mut XoroshiroRandom,
        pos: BlockPos,
    ) -> Option<VoxelId> {
        match self {
            StateProvider::Simple(state) => Some(*state),
            StateProvider::Weighted(entries) => {
                pick_weighted_by(entries, |(_, weight)| *weight, rng).map(|(state, _)| *state)
            }
            StateProvider::RuleBased { fallback, rules } => {
                for (predicate, then) in rules {
                    if predicate.test(volume, pos)
                        && let Some(state) = then.optional_state(volume, rng, pos)
                    {
                        return Some(state);
                    }
                }
                fallback
                    .as_ref()
                    .and_then(|provider| provider.optional_state(volume, rng, pos))
            }
            StateProvider::RandomizedInt {
                source,
                property,
                values,
            } => {
                let state = source.state(volume, rng, pos);
                let Some(property) = property.get(&volume.world().block_of(state)) else {
                    return Some(state);
                };
                Some(property.set(state, values.sample(rng)))
            }
            StateProvider::Rotated {
                source,
                direction,
                rotations,
            } => {
                let direction = direction.unwrap_or_else(|| random_direction(rng));
                let state = source.state(volume, rng, pos);
                Some(
                    rotations
                        .get(&volume.world().block_of(state))
                        .map_or(state, |rotations| rotations.rotate(state, direction)),
                )
            }
            StateProvider::RandomBlock(states) => {
                if states.is_empty() {
                    return None;
                }
                Some(states[rng.next_i32_bound(states.len() as i32) as usize])
            }
            StateProvider::CopyProperties(source) => {
                let state = source.state(volume, rng, pos);
                let here = volume.get(pos);
                let world = volume.world();
                Some(match (world.layout_of(state), world.layout_of(here)) {
                    (Some(into), Some(from)) => into.with_properties_of(state, from, here),
                    _ => state,
                })
            }
            StateProvider::Noise {
                noise,
                scale,
                states,
            } => Some(state_at_noise(
                states,
                noise_value(noise, pos, *scale as f64),
            )),
            StateProvider::NoiseThreshold {
                noise,
                scale,
                threshold,
                high_chance,
                default_state,
                low_states,
                high_states,
            } => {
                let local = noise_value(noise, pos, *scale as f64);
                if (local as f64) < *threshold as f64 {
                    return Some(low_states[rng.next_i32_bound(low_states.len() as i32) as usize]);
                }
                if rng.next_f32() < *high_chance {
                    return Some(
                        high_states[rng.next_i32_bound(high_states.len() as i32) as usize],
                    );
                }
                Some(*default_state)
            }
            StateProvider::DualNoise {
                slow_noise,
                slow_scale,
                variety_min,
                variety_max,
                noise,
                scale,
                states,
            } => {
                let variety = clamped_map(
                    slow_noise_value(slow_noise, pos, *slow_scale) as f64,
                    -1.0,
                    1.0,
                    *variety_min as f64,
                    (*variety_max + 1) as f64,
                ) as i32;
                let i = index_at_noise(
                    variety.max(0) as usize,
                    noise_value(noise, pos, *scale as f64),
                ) as i32;
                let offset = BlockPos::new(pos.x + i * 54545, pos.y, pos.z + i * 34234);
                Some(state_at_noise(
                    states,
                    slow_noise_value(slow_noise, offset, *slow_scale),
                ))
            }
        }
    }
}

/// The scale reaches the sampler as a `double`, so the products are `f64`.
fn noise_value(noise: &NoiseStack<Octave>, pos: BlockPos, scale: f64) -> f32 {
    noise.get(
        pos.x as f64 * scale,
        pos.y as f64 * scale,
        pos.z as f64 * scale,
    )
}

/// Unlike [`noise_value`], the slow scale is a `float` field multiplied against
/// an `int`, so the product rounds to `f32` before it widens.
fn slow_noise_value(noise: &NoiseStack<Octave>, pos: BlockPos, scale: f32) -> f32 {
    noise.get(
        (pos.x as f32 * scale) as f64,
        (pos.y as f32 * scale) as f64,
        (pos.z as f32 * scale) as f64,
    )
}

fn state_at_noise(states: &[VoxelId], noise: f32) -> VoxelId {
    states[index_at_noise(states.len(), noise)]
}

fn index_at_noise(len: usize, noise: f32) -> usize {
    let placement = ((1.0 + noise) / 2.0).clamp(0.0, 0.9999);
    (placement * len as f32) as usize
}

#[cfg(test)]
pub(crate) mod fake {
    use bevy_math::IVec3;
    use rustc_hash::FxHashMap as HashMap;

    use mcrs_minecraft_chunk::{Blocks, BlocksMut, Volume};
    use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
    use mcrs_minecraft_worldgen::feature::placer::WorldStates;
    use mcrs_minecraft_worldgen::value_provider::HeightContext;

    use super::*;

    pub(crate) const AIR: VoxelId = VoxelId(0);

    /// A volume over a sparse map of states, whose state algebra is arithmetic
    /// so a test can read the edit a provider asked for off the result. Its
    /// world knows `AIR` as the one air state and nothing else.
    pub(crate) struct FakeVolume {
        pub blocks: HashMap<(i32, i32, i32), VoxelId>,
        pub heights: HashMap<(i32, i32), i32>,
        pub writes: Vec<((i32, i32, i32), VoxelId)>,
        pub world: WorldStates,
    }

    impl Default for FakeVolume {
        fn default() -> Self {
            let mut air_states = fixedbitset::FixedBitSet::with_capacity(1);
            air_states.insert(AIR.0 as usize);
            FakeVolume {
                blocks: HashMap::default(),
                heights: HashMap::default(),
                writes: Vec::new(),
                world: WorldStates {
                    air: AIR,
                    air_states: Arc::new(air_states),
                    ..WorldStates::default()
                },
            }
        }
    }

    impl FakeVolume {
        pub fn with(blocks: impl IntoIterator<Item = ((i32, i32, i32), VoxelId)>) -> Self {
            FakeVolume {
                blocks: blocks.into_iter().collect(),
                ..FakeVolume::default()
            }
        }
    }

    impl Volume for FakeVolume {
        fn min(&self) -> BlockPos {
            IVec3::splat(i32::MIN / 2).into()
        }

        fn max(&self) -> BlockPos {
            IVec3::splat(i32::MAX / 2).into()
        }
    }

    impl Blocks for FakeVolume {
        fn get(&self, p: BlockPos) -> VoxelId {
            self.blocks.get(&(p.x, p.y, p.z)).copied().unwrap_or(AIR)
        }
    }

    impl BlocksMut for FakeVolume {
        fn set(&mut self, p: BlockPos, state: VoxelId) {
            self.blocks.insert((p.x, p.y, p.z), state);
            self.writes.push(((p.x, p.y, p.z), state));
        }
    }

    impl WorldGenVolume for FakeVolume {
        fn world(&self) -> &WorldStates {
            &self.world
        }

        fn height(&self, _kind: HeightmapName, x: i32, z: i32) -> i32 {
            self.heights.get(&(x, z)).copied().unwrap_or(i32::MIN)
        }

        fn biome(&self, _: BlockPos) -> u32 {
            0
        }

        fn extent(&self) -> HeightContext {
            HeightContext {
                min_y: -64,
                depth: 384,
                sea_level: 63,
            }
        }

        fn would_survive(&self, _: VoxelId, _: BlockPos) -> bool {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::IVec3;
    use mcrs_minecraft_worldgen::feature::placer::mask_of;
    use std::sync::Arc;

    use mcrs_minecraft_chunk::BlocksMut;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::feature::placer::PropertyLayout;
    use mcrs_minecraft_worldgen::noise::normal;
    use mcrs_minecraft_worldgen::value_provider::{DispatchedIntProvider, IntProvider};

    use super::fake::{AIR, FakeVolume};
    use super::*;

    const A: VoxelId = VoxelId(11);
    const B: VoxelId = VoxelId(22);
    const C: VoxelId = VoxelId(33);
    const ORIGIN: BlockPos = BlockPos::new(4, 70, -9);

    fn rng() -> XoroshiroRandom {
        XoroshiroRandom::new(0x5eed_1234)
    }

    /// The provider under test, then the draw sequence it is claimed to spend,
    /// replayed from the same start: equal states mean the claim holds.
    fn assert_draws(
        provider: &StateProvider,
        volume: &FakeVolume,
        expected: impl Fn(&mut XoroshiroRandom),
    ) -> VoxelId {
        let mut actual = rng();
        let state = provider.state(volume, &mut actual, ORIGIN);
        let mut replay = rng();
        expected(&mut replay);
        assert_eq!(actual, replay, "draw sequence");
        state
    }

    #[test]
    fn simple_draws_nothing() {
        let volume = FakeVolume::default();
        assert_eq!(assert_draws(&StateProvider::Simple(A), &volume, |_| {}), A);
    }

    #[test]
    fn weighted_draws_one_int_and_walks_the_cumulative_weights() {
        let volume = FakeVolume::default();
        let provider = StateProvider::Weighted(vec![(A, 1), (B, 0), (C, 3)]);
        let state = assert_draws(&provider, &volume, |replay| {
            replay.next_i32_bound(4);
        });

        let selection = rng().next_i32_bound(4);
        assert_eq!(state, if selection < 1 { A } else { C });
        assert_eq!(
            (0..4)
                .map(|selection| {
                    let mut walked = selection;
                    for (state, weight) in [(A, 1), (B, 0), (C, 3)] {
                        walked -= weight;
                        if walked < 0 {
                            return state;
                        }
                    }
                    unreachable!()
                })
                .collect::<Vec<_>>(),
            [A, C, C, C],
            "a zero weight is never selected"
        );
    }

    #[test]
    fn rule_based_draws_only_inside_the_branch_it_takes() {
        let volume = FakeVolume::with([((4, 70, -9), B)]);
        let provider = StateProvider::RuleBased {
            fallback: Some(Box::new(StateProvider::Weighted(vec![(C, 5)]))),
            rules: vec![
                (
                    Predicate::MatchingStates {
                        offset: IVec3::ZERO,
                        states: mask_of([A]),
                    },
                    StateProvider::Weighted(vec![(A, 7)]),
                ),
                (
                    Predicate::MatchingStates {
                        offset: IVec3::ZERO,
                        states: mask_of([B]),
                    },
                    StateProvider::Simple(A),
                ),
            ],
        };
        assert_eq!(assert_draws(&provider, &volume, |_| {}), A);
    }

    #[test]
    fn rule_based_falls_back_when_no_rule_matches() {
        let volume = FakeVolume::with([((4, 70, -9), C)]);
        let provider = StateProvider::RuleBased {
            fallback: Some(Box::new(StateProvider::Weighted(vec![(B, 5)]))),
            rules: vec![(
                Predicate::MatchingStates {
                    offset: IVec3::ZERO,
                    states: mask_of([A]),
                },
                StateProvider::Simple(A),
            )],
        };
        let state = assert_draws(&provider, &volume, |replay| {
            replay.next_i32_bound(5);
        });
        assert_eq!(state, B);
    }

    /// No fallback and no matching rule is the one case that reads the volume
    /// back instead of deciding a state.
    #[test]
    fn rule_based_without_a_fallback_keeps_the_block_that_is_there() {
        let volume = FakeVolume::with([((4, 70, -9), C)]);
        let provider = StateProvider::RuleBased {
            fallback: None,
            rules: Vec::new(),
        };
        assert_eq!(assert_draws(&provider, &volume, |_| {}), C);
    }

    /// One block over states `0..24`: `axis` in `x, y, z` eight ids apart,
    /// then `age` in `0..8`.
    fn laid_out() -> FakeVolume {
        let text =
            |values: &[&str]| -> Arc<[Arc<str>]> { values.iter().map(|v| Arc::from(*v)).collect() };
        let mut volume = FakeVolume::default();
        volume.world.block_of_state = (0..64).map(|_| 0).collect();
        volume.world.layouts = Arc::new([BlockLayout {
            base: 0,
            properties: vec![
                PropertyLayout {
                    name: Arc::from("axis"),
                    values: text(&["x", "y", "z"]),
                    stride: 8,
                },
                PropertyLayout {
                    name: Arc::from("age"),
                    values: text(&["0", "1", "2", "3", "4", "5", "6", "7"]),
                    stride: 1,
                },
            ],
        }]);
        volume
    }

    #[test]
    fn randomized_int_draws_the_value_only_when_the_property_is_there() {
        let volume = laid_out();
        let values = IntProvider::Dispatched(DispatchedIntProvider::Uniform {
            min_inclusive: 2,
            max_inclusive: 5,
        });
        let table = |name: &str| Arc::new(int_property_table(&volume.world.layouts, name));
        let present = StateProvider::RandomizedInt {
            source: Box::new(StateProvider::Simple(A)),
            property: table("age"),
            values: values.clone(),
        };
        let state = assert_draws(&present, &volume, |replay| {
            replay.next_int_between_inclusive(2, 5);
        });
        assert_eq!(
            state.0,
            A.0 - 3 + rng().next_int_between_inclusive(2, 5) as u16,
            "the age digit of A is 3"
        );

        let absent = StateProvider::RandomizedInt {
            source: Box::new(StateProvider::Simple(A)),
            property: table("level"),
            values,
        };
        assert_eq!(assert_draws(&absent, &volume, |_| {}), A);
    }

    #[test]
    fn rotated_draws_a_direction_only_when_the_asset_names_none() {
        let volume = laid_out();
        let rotations = Arc::new(rotation_table(&volume.world.layouts));
        let free = StateProvider::Rotated {
            source: Box::new(StateProvider::Simple(A)),
            direction: None,
            rotations: rotations.clone(),
        };
        let state = assert_draws(&free, &volume, |replay| {
            replay.next_i32_bound(6);
        });
        let drawn = random_direction(&mut rng());
        assert_eq!(
            state.0,
            A.0 - 8 + 8 * drawn.axis() as u16,
            "the axis digit of A is y"
        );

        let fixed = StateProvider::Rotated {
            source: Box::new(StateProvider::Simple(A)),
            direction: Some(Direction::West),
            rotations,
        };
        assert_eq!(assert_draws(&fixed, &volume, |_| {}).0, A.0 - 8);
    }

    #[test]
    fn random_block_draws_one_int_and_declines_when_empty() {
        let volume = FakeVolume::with([((4, 70, -9), C)]);
        let provider = StateProvider::RandomBlock(vec![A, B]);
        let state = assert_draws(&provider, &volume, |replay| {
            replay.next_i32_bound(2);
        });
        assert_eq!(state, [A, B][rng().next_i32_bound(2) as usize]);

        let empty = StateProvider::RandomBlock(Vec::new());
        assert_eq!(assert_draws(&empty, &volume, |_| {}), C);
    }

    #[test]
    fn copy_properties_reads_the_block_that_is_there() {
        let mut volume = laid_out();
        volume.set(ORIGIN, B);
        let provider = StateProvider::CopyProperties(Box::new(StateProvider::Simple(A)));
        assert_eq!(
            assert_draws(&provider, &volume, |_| {}),
            B,
            "A and B are one block, so every digit is copied"
        );
    }

    fn noise(base_octave: i32, seed: u64) -> SharedNoise {
        Arc::new(normal::create_parity(
            base_octave,
            &[1.0],
            &mut LegacyRandom::new(seed),
        ))
    }

    #[test]
    fn the_noise_providers_draw_nothing() {
        let volume = FakeVolume::default();
        let states = vec![A, B, C];
        let provider = StateProvider::Noise {
            noise: noise(0, 2345),
            scale: 0.020_833_334,
            states: states.clone(),
        };
        let picked = assert_draws(&provider, &volume, |_| {});
        assert!(states.contains(&picked));

        let dual = StateProvider::DualNoise {
            slow_noise: noise(-10, 2345),
            slow_scale: 1.0,
            variety_min: 1,
            variety_max: 3,
            noise: noise(-3, 2345),
            scale: 1.0,
            states,
        };
        let picked_dual = assert_draws(&dual, &volume, |_| {});
        assert!([A, B, C].contains(&picked_dual));
    }

    #[test]
    fn noise_threshold_spends_one_draw_below_and_two_above() {
        let volume = FakeVolume::default();
        let below = StateProvider::NoiseThreshold {
            noise: noise(0, 2345),
            scale: 1.0,
            threshold: 1.0,
            high_chance: 1.0,
            default_state: AIR,
            low_states: vec![A, B],
            high_states: vec![C],
        };
        let state = assert_draws(&below, &volume, |replay| {
            replay.next_i32_bound(2);
        });
        assert_eq!(state, [A, B][rng().next_i32_bound(2) as usize]);

        let above = StateProvider::NoiseThreshold {
            noise: noise(0, 2345),
            scale: 1.0,
            threshold: -1.0,
            high_chance: 1.0,
            default_state: AIR,
            low_states: vec![A],
            high_states: vec![B, C],
        };
        let high = assert_draws(&above, &volume, |replay| {
            replay.next_f32();
            replay.next_i32_bound(2);
        });
        let mut expect = rng();
        expect.next_f32();
        assert_eq!(high, [B, C][expect.next_i32_bound(2) as usize]);

        let default = StateProvider::NoiseThreshold {
            noise: noise(0, 2345),
            scale: 1.0,
            threshold: -1.0,
            high_chance: 0.0,
            default_state: C,
            low_states: vec![A],
            high_states: vec![B],
        };
        let state = assert_draws(&default, &volume, |replay| {
            replay.next_f32();
        });
        assert_eq!(state, C);
    }
}
