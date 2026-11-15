use glam::DVec3;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_predicate::{
    BakedBlockPredicate, BakedEntityPredicate, BakedItemPredicate, BakedLocationPredicate,
    ItemSubject, PredicateScratch, PredicateTables,
};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::Id;

use crate::context_param::ContextParam;
use crate::context_params::ContextParams;
use crate::float_provider::FloatProvider;
use crate::loot_inputs::LootInputs;
use crate::loot_world::LootWorld;
use crate::pending_test::PendingTest;

/// A baked loot condition. Its test takes the draws of its rule in `loot-conditions.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition(pub(crate) Test);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Test {
    /// A condition with a part that the bake could not resolve.
    Never,
    /// The entry of the registry `predicate` at the index, with what it reads.
    Named {
        index: usize,
        reads: ContextParams,
    },
    Inverted(Box<Test>),
    AnyOf(Box<[Test]>),
    AllOf(Box<[Test]>),
    SurvivesExplosion,
    RandomChance(FloatProvider),
    TableBonus {
        enchantment: Option<Id<Enchantment>>,
        chances: Box<[f32]>,
    },
    MatchBlock {
        predicate: Box<BakedBlockPredicate>,
        reads: ContextParams,
    },
    MatchTool(Option<Box<BakedItemPredicate>>),
    LocationCheck {
        offset: DVec3,
        predicate: Option<Box<BakedLocationPredicate>>,
    },
    EntityProperties {
        target: ContextParam,
        predicate: Option<Box<BakedEntityPredicate>>,
    },
    Pending(Box<PendingTest>),
}

impl Condition {
    /// Tests the condition with `inputs`, the draws from `random` and the buffers of `scratch`.
    pub fn test<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> bool {
        self.0.test(inputs, random, scratch)
    }

    /// The parameters that the condition reads, for the check of LOOT-CTX-013.
    #[must_use]
    pub fn reads(&self) -> ContextParams {
        self.0.reads()
    }
}

impl Test {
    pub(crate) fn test<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let LootInputs {
            named,
            context,
            world,
        } = inputs;
        match self {
            Test::Never => false,
            Test::Named { index, .. } => named
                .predicates
                .get(*index)
                .is_some_and(|entry| entry.node.test(inputs, random, scratch)),
            Test::Inverted(term) => !term.test(inputs, random, scratch),
            Test::AnyOf(terms) => terms.iter().any(|term| term.test(inputs, random, scratch)),
            Test::AllOf(terms) => terms.iter().all(|term| term.test(inputs, random, scratch)),
            Test::SurvivesExplosion => context
                .explosion_radius
                .is_none_or(|radius| random.next_f32() <= 1.0 / radius),
            Test::RandomChance(chance) => {
                let chance = chance.read(inputs, random, scratch);
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
            Test::MatchBlock { predicate, .. } => context
                .block_state
                .is_some_and(|state| predicate.matches_state(Some(state), context.block_entity)),
            Test::MatchTool(predicate) => context.tool.is_some_and(|tool| {
                predicate.as_ref().is_none_or(|predicate| {
                    predicate.matches(ItemSubject::from(tool), &named.items)
                })
            }),
            Test::LocationCheck { offset, predicate } => context.origin.is_some_and(|origin| {
                let Some(predicate) = predicate else {
                    return true;
                };
                named
                    .smoke
                    .as_ref()
                    .is_ok_and(|smoke| predicate.matches(world, origin + *offset, smoke, scratch))
            }),
            Test::EntityProperties { target, predicate } => {
                let Some(predicate) = predicate else {
                    return true;
                };
                named.smoke.as_ref().is_ok_and(|smoke| {
                    let entity = context.entity(*target).copied();
                    let tables = PredicateTables::new(smoke, &named.items);
                    predicate.matches(world, context.origin, entity, tables, scratch)
                })
            }
            // chisle: each type of `PendingTest` fails; the context parameters and the world
            // views of those types lift it.
            Test::Pending(_) => false,
        }
    }

    pub(crate) fn reads(&self) -> ContextParams {
        match self {
            Test::Never => ContextParams::EMPTY,
            Test::Named { reads, .. } | Test::MatchBlock { reads, .. } => *reads,
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
            Test::LocationCheck { .. } => ContextParams::of(&[ContextParam::Origin]),
            Test::EntityProperties { target, .. } => {
                ContextParams::of(&[*target, ContextParam::Origin])
            }
            Test::Pending(pending) => pending.reads(),
        }
    }
}
