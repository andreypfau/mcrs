use core::num::NonZeroI64;

use mcrs_minecraft_enchantment::level_based_value::LevelBasedValue;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::{AttributeValue, WorldClock, info};
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_predicate::BakedDamageSourcePredicate;
use mcrs_minecraft_resource::Id;

use crate::context_param::ContextParam;
use crate::context_params::ContextParams;
use crate::float_provider::FloatProvider;
use crate::int_provider::{IntProvider, environment_reads, union};
use crate::number_range::NumberRange;

/// A condition type that the bake resolves and checks, and whose test always fails.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PendingTest {
    RandomChanceWithEnchantedBonus {
        enchantment: Option<Id<Enchantment>>,
        unenchanted_chance: f32,
        enchanted_chance: LevelBasedValue,
    },
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
    TimeCheck {
        clock: Option<Id<WorldClock>>,
        period: Option<NonZeroI64>,
        value: NumberRange<IntProvider>,
    },
    IntValueCheck {
        value: IntProvider,
        test: NumberRange<IntProvider>,
    },
    FloatValueCheck {
        value: FloatProvider,
        test: NumberRange<FloatProvider>,
    },
    EnchantmentActiveCheck {
        active: bool,
    },
    EnvironmentAttributeCheck {
        attribute: EnvironmentAttribute,
        value: AttributeValue,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ObjectiveTest {
    pub(crate) objective: Box<str>,
    pub(crate) range: NumberRange<IntProvider>,
}

impl PendingTest {
    pub(crate) fn reads(&self) -> ContextParams {
        match self {
            PendingTest::RandomChanceWithEnchantedBonus { .. } => {
                ContextParams::of(&[ContextParam::AttackingEntity])
            }
            PendingTest::KilledByPlayer => ContextParams::of(&[ContextParam::LastDamagePlayer]),
            PendingTest::EntityScores { target, scores } => union(
                scores
                    .iter()
                    .flat_map(|score| score.range.providers())
                    .map(IntProvider::reads),
            )
            .with(*target),
            PendingTest::DamageSourceProperties(_) => {
                ContextParams::of(&[ContextParam::DamageSource, ContextParam::Origin])
            }
            PendingTest::WeatherCheck { .. } => ContextParams::EMPTY,
            PendingTest::TimeCheck { value, .. } => {
                union(value.providers().map(IntProvider::reads))
            }
            PendingTest::IntValueCheck { value, test } => {
                union(test.providers().map(IntProvider::reads)).union(value.reads())
            }
            PendingTest::FloatValueCheck { value, test } => {
                union(test.providers().map(FloatProvider::reads)).union(value.reads())
            }
            PendingTest::EnchantmentActiveCheck { .. } => {
                ContextParams::of(&[ContextParam::EnchantmentActive])
            }
            PendingTest::EnvironmentAttributeCheck { attribute, .. } => {
                environment_reads(info(*attribute).positional)
            }
        }
    }
}
