use glam::DVec3;
use mcrs_minecraft_environment::info;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_predicate::{
    BakedBlockPredicate, BakedDamageSourcePredicate, BakedEntityPredicate, BakedItemPredicate,
    BakedLocationPredicate, ItemSubject, PredicateScratch, PredicateTables,
};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::Id;

use crate::attribute_check::AttributeCheck;
use crate::context_param::ContextParam;
use crate::context_params::ContextParams;
use crate::enchanted_bonus::EnchantedBonus;
use crate::float_provider::FloatProvider;
use crate::int_provider::{IntProvider, environment_reads, union};
use crate::loot_inputs::LootInputs;
use crate::loot_world::LootWorld;
use crate::objective_test::ObjectiveTest;
use crate::score_view::ScoreHolder;
use crate::time_check::TimeCheck;
use crate::value_check::ValueCheck;
use crate::weather_view::WeatherView;

#[derive(Debug, Clone, PartialEq)]
pub struct Condition(pub(crate) Test);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Test {
    Never,
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
    EnchantedBonus(Box<EnchantedBonus>),
    KilledByPlayer,
    EntityScores {
        target: ContextParam,
        scores: Box<[ObjectiveTest]>,
    },
    DamageSourceProperties(Option<Box<BakedDamageSourcePredicate>>),
    WeatherCheck {
        raining: Option<bool>,
        thundering: Option<bool>,
    },
    TimeCheck(Box<TimeCheck>),
    IntValueCheck(Box<ValueCheck<IntProvider>>),
    FloatValueCheck(Box<ValueCheck<FloatProvider>>),
    EnchantmentActiveCheck {
        active: bool,
    },
    EnvironmentAttributeCheck(Box<AttributeCheck>),
}

impl Condition {
    pub fn test<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> bool {
        self.0.test(inputs, random, scratch)
    }

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
            Test::EnchantedBonus(bonus) => {
                let chance = bonus.chance(world, context.attacking_entity);
                random.next_f32() < chance
            }
            Test::KilledByPlayer => context.last_damage_player.is_some(),
            Test::EntityScores { target, scores } => {
                entity_scores(*target, scores, inputs, random, scratch)
            }
            Test::DamageSourceProperties(predicate) => {
                damage_source_properties(predicate.as_deref(), inputs, scratch)
            }
            Test::WeatherCheck {
                raining,
                thundering,
            } => weather_check(*raining, *thundering, world),
            Test::TimeCheck(check) => check.test(inputs, random, scratch),
            Test::IntValueCheck(check) => {
                check
                    .test
                    .contains_read(&check.value, inputs, random, scratch)
            }
            Test::FloatValueCheck(check) => {
                check
                    .test
                    .contains_read(&check.value, inputs, random, scratch)
            }
            Test::EnchantmentActiveCheck { active } => context.enchantment_active == Some(*active),
            Test::EnvironmentAttributeCheck(check) => check.matches(world, context.origin),
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
            Test::EnchantedBonus(_) => ContextParams::of(&[ContextParam::AttackingEntity]),
            Test::KilledByPlayer => ContextParams::of(&[ContextParam::LastDamagePlayer]),
            Test::EntityScores { target, scores } => union(
                scores
                    .iter()
                    .flat_map(|score| score.range.providers())
                    .map(IntProvider::reads),
            )
            .with(*target),
            Test::DamageSourceProperties(_) => {
                ContextParams::of(&[ContextParam::DamageSource, ContextParam::Origin])
            }
            Test::WeatherCheck { .. } => ContextParams::EMPTY,
            Test::TimeCheck(check) => union(check.value.providers().map(IntProvider::reads)),
            Test::IntValueCheck(check) => {
                union(check.test.providers().map(IntProvider::reads)).union(check.value.reads())
            }
            Test::FloatValueCheck(check) => {
                union(check.test.providers().map(FloatProvider::reads)).union(check.value.reads())
            }
            Test::EnchantmentActiveCheck { .. } => {
                ContextParams::of(&[ContextParam::EnchantmentActive])
            }
            Test::EnvironmentAttributeCheck(check) => {
                environment_reads(info(check.attribute).positional)
            }
        }
    }
}

fn entity_scores<E: Copy, W: LootWorld<E>, R: Random>(
    target: ContextParam,
    scores: &[ObjectiveTest],
    inputs: LootInputs<'_, '_, E, W>,
    random: &mut R,
    scratch: &mut PredicateScratch,
) -> bool {
    let Some(entity) = inputs.context.entity(target) else {
        return false;
    };
    scores.iter().all(|score| {
        inputs
            .world
            .score(ScoreHolder::Entity(entity), &score.objective)
            .is_some_and(|value| score.range.contains(value, inputs, random, scratch))
    })
}

fn damage_source_properties<E: Copy, W: LootWorld<E>>(
    predicate: Option<&BakedDamageSourcePredicate>,
    inputs: LootInputs<'_, '_, E, W>,
    scratch: &mut PredicateScratch,
) -> bool {
    let LootInputs {
        named,
        context,
        world,
    } = inputs;
    let (Some(source), Some(origin)) = (context.damage_source, context.origin) else {
        return false;
    };
    let Some(predicate) = predicate else {
        return true;
    };
    named.smoke.as_ref().is_ok_and(|smoke| {
        let tables = PredicateTables::new(smoke, &named.items);
        predicate.matches(world, Some(origin), Some(source), tables, scratch)
    })
}

fn weather_check<W: WeatherView>(
    raining: Option<bool>,
    thundering: Option<bool>,
    world: &W,
) -> bool {
    let rain = world.rain_strength().unwrap_or(0.0);
    let thunder = world.thunder_strength().unwrap_or(0.0);
    let rains = rain > 0.2;
    let thunders = thunder * rain > 0.9;
    raining.is_none_or(|raining| raining == rains)
        && thundering.is_none_or(|thundering| thundering == thunders)
}
