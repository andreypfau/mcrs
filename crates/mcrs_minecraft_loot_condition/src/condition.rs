use glam::DVec3;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::context_param::ContextParam;
use crate::context_params::ContextParams;
use crate::float_provider::FloatProvider;
use crate::loot_context::LootContext;
use crate::loot_world::LootWorld;
use crate::named_providers::NamedProviders;
use crate::tool::Tool;

/// A baked loot condition. Its test takes the draws of its rule in `loot-conditions.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition(pub(crate) Test);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Test {
    /// A condition of a type or with a part that the bake does not evaluate.
    Never,
    Inverted(Box<Test>),
    AnyOf(Box<[Test]>),
    AllOf(Box<[Test]>),
    SurvivesExplosion,
    RandomChance(FloatProvider),
    TableBonus {
        enchantment: Option<Id<Enchantment>>,
        chances: Box<[f32]>,
    },
    StateIn(StateSet),
    BlockAt {
        offset: DVec3,
        states: Option<StateSet>,
    },
    MatchTool(ItemTest),
    /// `entity_properties` of `this` with the predicate `{}`.
    ThisEntityPresent,
    /// `entity_properties` without a predicate, of the target that names the parameter.
    EntityUntested(ContextParam),
}

impl Condition {
    /// Tests the condition in `context`, with the draws from `random`. `providers` are the
    /// named providers that the bake resolved the keys against.
    pub fn test<E, W: LootWorld<E>, R: Random>(
        &self,
        providers: &NamedProviders,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
    ) -> bool {
        self.0.test(providers, context, world, random)
    }

    /// The parameters that the condition reads, for the check of LOOT-CTX-013.
    #[must_use]
    pub fn reads(&self) -> ContextParams {
        self.0.reads()
    }
}

impl Test {
    fn test<E, W: LootWorld<E>, R: Random>(
        &self,
        providers: &NamedProviders,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
    ) -> bool {
        match self {
            Test::Never => false,
            Test::Inverted(term) => !term.test(providers, context, world, random),
            Test::AnyOf(terms) => terms
                .iter()
                .any(|term| term.test(providers, context, world, random)),
            Test::AllOf(terms) => terms
                .iter()
                .all(|term| term.test(providers, context, world, random)),
            Test::SurvivesExplosion => context
                .explosion_radius
                .is_none_or(|radius| random.next_f32() <= 1.0 / radius),
            Test::RandomChance(chance) => {
                let chance = chance.read(providers, context, world, random);
                random.next_f32() < chance
            }
            Test::TableBonus {
                enchantment,
                chances,
            } => {
                let level = context
                    .tool
                    .zip(*enchantment)
                    .map_or(0, |(tool, enchantment)| tool.level(enchantment));
                let level = usize::try_from(level).unwrap_or(usize::MAX);
                let last = chances.len().saturating_sub(1);
                chances
                    .get(level.min(last))
                    .is_some_and(|&chance| random.next_f32() < chance)
            }
            Test::StateIn(states) => context
                .block_state
                .is_some_and(|state| states.contains(state)),
            Test::BlockAt { offset, states } => context.origin.is_some_and(|origin| {
                states.as_ref().is_none_or(|states| {
                    world
                        .block_state(BlockPos::from(origin + *offset))
                        .is_some_and(|state| states.contains(state))
                })
            }),
            Test::MatchTool(test) => context.tool.is_some_and(|tool| test.matches(&tool)),
            Test::ThisEntityPresent => context.this_entity.is_some(),
            Test::EntityUntested(_) => true,
        }
    }

    fn reads(&self) -> ContextParams {
        match self {
            Test::Never => ContextParams::EMPTY,
            Test::Inverted(term) => term.reads(),
            Test::AnyOf(terms) | Test::AllOf(terms) => {
                terms.iter().fold(ContextParams::EMPTY, |reads, term| {
                    reads.union(term.reads())
                })
            }
            Test::SurvivesExplosion => ContextParams::of(&[ContextParam::ExplosionRadius]),
            Test::RandomChance(chance) => chance.reads(),
            Test::TableBonus { .. } | Test::MatchTool(_) => {
                ContextParams::of(&[ContextParam::Tool])
            }
            Test::StateIn(_) => ContextParams::of(&[ContextParam::BlockState]),
            Test::BlockAt { .. } => ContextParams::of(&[ContextParam::Origin]),
            Test::ThisEntityPresent => {
                ContextParams::of(&[ContextParam::ThisEntity, ContextParam::Origin])
            }
            Test::EntityUntested(param) => ContextParams::of(&[*param, ContextParam::Origin]),
        }
    }
}

/// The states a block predicate accepts, sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateSet(pub(crate) Box<[BlockStateId]>);

impl StateSet {
    fn contains(&self, state: BlockStateId) -> bool {
        self.0.binary_search(&state).is_ok()
    }
}

/// The parts of an item predicate that block tables use (LOOT-CND-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemTest {
    pub(crate) items: Option<Box<[Id<Item>]>>,
    pub(crate) enchantments: Box<[EnchantmentTest]>,
}

/// One element of the sub-predicate `minecraft:enchantments`: any of `any_of`, or any
/// enchantment when it is `None`, at a level in `min..=max`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnchantmentTest {
    pub(crate) any_of: Option<Box<[Id<Enchantment>]>>,
    pub(crate) min: u32,
    pub(crate) max: u32,
}

impl ItemTest {
    fn matches(&self, tool: &Tool<'_>) -> bool {
        let item_matches = match (&self.items, tool.item()) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(items), Some(kind)) => items.iter().any(|item| item.number() == kind.0),
        };
        item_matches
            && self.enchantments.iter().all(|test| {
                tool.enchantments().iter().any(|entry| {
                    let level = u32::from(entry.level.get());
                    level >= test.min.max(1)
                        && level <= test.max
                        && test.any_of.as_ref().is_none_or(|ids| {
                            ids.iter().any(|id| id.number() == entry.enchantment.0)
                        })
                })
            })
    }
}
