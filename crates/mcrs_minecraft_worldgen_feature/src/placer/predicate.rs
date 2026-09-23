use std::sync::Arc;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;

use mcrs_minecraft_core::value_provider::VerticalAnchor;

use super::{BiomeMask, StateMask, WorldGenVolume};

/// A `BlockPredicate` with every set it names reduced to a mask.
#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    True,
    MatchingStates {
        offset: IVec3,
        states: StateMask,
    },
    MatchingBiomes(BiomeMask),
    WouldSurvive {
        offset: IVec3,
        state: VoxelId,
    },
    InsideWorldBounds {
        offset_y: i32,
    },
    HeightRange {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    VolumeMatch {
        min: [i32; 3],
        max: [i32; 3],
        matches: Box<Predicate>,
    },
    AnyOf(Vec<Predicate>),
    AllOf(Vec<Predicate>),
    Not(Box<Predicate>),
}

impl Predicate {
    pub fn test<W: WorldGenVolume>(&self, volume: &W, pos: BlockPos) -> bool {
        match self {
            Predicate::True => true,
            Predicate::MatchingStates { offset, states } => {
                states.contains(volume.get(pos + *offset).0 as usize)
            }
            Predicate::MatchingBiomes(biomes) => biomes.contains(volume.biome(pos) as usize),
            Predicate::WouldSurvive { offset, state } => {
                volume.would_survive(*state, pos + *offset)
            }
            Predicate::InsideWorldBounds { offset_y } => volume.extent().contains(pos.y + offset_y),
            Predicate::HeightRange {
                min_inclusive,
                max_inclusive,
            } => {
                let extent = volume.extent();
                pos.y >= min_inclusive.resolve_y(extent) && pos.y <= max_inclusive.resolve_y(extent)
            }
            Predicate::VolumeMatch { min, max, matches } => {
                for x in min[0]..=max[0] {
                    for z in min[2]..=max[2] {
                        for y in min[1]..=max[1] {
                            if !matches.test(volume, pos + IVec3::new(x, y, z)) {
                                return false;
                            }
                        }
                    }
                }
                true
            }
            Predicate::AnyOf(predicates) => predicates.iter().any(|p| p.test(volume, pos)),
            Predicate::AllOf(predicates) => predicates.iter().all(|p| p.test(volume, pos)),
            Predicate::Not(predicate) => !predicate.test(volume, pos),
        }
    }
}

/// A `RuleTest` with every block, block state and tag it names reduced to a
/// mask.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    AlwaysTrue,
    MatchingStates(StateMask),
    HeightMatch {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    /// The membership test short-circuits the draw: a state outside the mask
    /// never reaches `nextFloat`.
    RandomStates {
        states: StateMask,
        probability: f32,
    },
    AllOf(Vec<Rule>),
    AnyOf(Vec<Rule>),
    Not(Box<Rule>),
}

pub fn mask_of(states: impl IntoIterator<Item = impl Into<u16>>) -> StateMask {
    Arc::new(
        states
            .into_iter()
            .map(|state| usize::from(state.into()))
            .collect(),
    )
}

pub fn single_state(state: VoxelId) -> StateMask {
    mask_of([state.0])
}

impl Rule {
    pub fn test<R: Random>(&self, state: VoxelId, y: i32, rng: &mut R) -> bool {
        match self {
            Rule::AlwaysTrue => true,
            Rule::MatchingStates(states) => states.contains(state.0 as usize),
            Rule::HeightMatch {
                min_inclusive,
                max_inclusive,
            } => *min_inclusive <= y && y <= *max_inclusive,
            Rule::RandomStates {
                states,
                probability,
            } => states.contains(state.0 as usize) && rng.next_f32() < *probability,
            Rule::AllOf(rules) => rules.iter().all(|rule| rule.test(state, y, rng)),
            Rule::AnyOf(rules) => rules.iter().any(|rule| rule.test(state, y, rng)),
            Rule::Not(rule) => !rule.test(state, y, rng),
        }
    }
}
