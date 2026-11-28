use core::num::NonZeroI64;

use mcrs_minecraft_enchantment::file::EnchantmentFile;
use mcrs_minecraft_enchantment::level_based_value::LevelBasedValue;
use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_predicate::DamageSourcePredicate;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf};

use crate::condition::Test;
use crate::condition_baker::ConditionBaker;
use crate::condition_problem::ConditionProblem;
use crate::enchanted_bonus::{EnchantedBonus, equipment_slots};
use crate::entity_target::EntityTarget;
use crate::float_provider::FloatProvider;
use crate::float_provider_file::FloatProviderFile;
use crate::int_provider::IntProvider;
use crate::int_provider_file::IntProviderFile;
use crate::number_range::NumberRange;
use crate::objective_test::ObjectiveTest;
use crate::range_file::RangeFile;
use crate::scores_file::ScoresFile;
use crate::time_check::TimeCheck;
use crate::value_check::ValueCheck;

impl ConditionBaker<'_> {
    pub(crate) fn enchanted_bonus(
        &mut self,
        enchantment: ResourceLocation<'_>,
        unenchanted_chance: f32,
        enchanted_chance: &LevelBasedValue,
    ) -> Test {
        let enchantment = self.enchantment(enchantment);
        let slots = enchantment
            .and_then(|id| self.registries.registry::<EnchantmentFile>()?.get(id))
            .map(|file| equipment_slots(&file.slots))
            .unwrap_or_default();
        Test::EnchantedBonus(Box::new(EnchantedBonus {
            enchantment,
            slots,
            unenchanted_chance,
            enchanted_chance: enchanted_chance.clone(),
        }))
    }

    pub(crate) fn entity_scores(&mut self, entity: EntityTarget, scores: &ScoresFile) -> Test {
        Test::EntityScores {
            target: entity.param(),
            scores: scores
                .0
                .iter()
                .map(|score| ObjectiveTest {
                    objective: score.objective.as_str().into(),
                    range: self.int_range(&format!("scores.{}", score.objective), &score.range),
                })
                .collect(),
        }
    }

    pub(crate) fn damage_source_properties(
        &mut self,
        predicate: Option<&DamageSourcePredicate<Named>>,
    ) -> Option<Test> {
        let Some(named) = predicate else {
            return Some(Test::DamageSourceProperties(None));
        };
        self.needs_smoke()?;
        let predicate = self.resolved::<DamageSourcePredicate>(Some("predicate"), named)?;
        let baked = predicate.bake(self.registries, self.definitions, &self.items);
        let baked = self.baked(baked)?;
        Some(Test::DamageSourceProperties(Some(Box::new(baked))))
    }

    pub(crate) fn time_check(
        &mut self,
        clock: &ResourceLocationBuf,
        period: Option<NonZeroI64>,
        value: &RangeFile<IntProviderFile>,
    ) -> Test {
        let id = self.lookup::<WorldClock>(clock.as_location());
        if id.is_none() {
            self.problems
                .push(ConditionProblem::UnknownClock(clock.to_string()));
        }
        Test::TimeCheck(Box::new(TimeCheck {
            clock: id,
            period,
            value: self.int_range("value", value),
        }))
    }

    pub(crate) fn int_value_check(
        &mut self,
        value: &IntProviderFile,
        test: &RangeFile<IntProviderFile>,
    ) -> Test {
        Test::IntValueCheck(Box::new(ValueCheck {
            value: self.condition_int("value", value),
            test: self.int_range("test", test),
        }))
    }

    pub(crate) fn float_value_check(
        &mut self,
        value: &FloatProviderFile,
        test: &RangeFile<FloatProviderFile>,
    ) -> Test {
        Test::FloatValueCheck(Box::new(ValueCheck {
            value: self.condition_float("value", value),
            test: self.float_range("test", test),
        }))
    }

    fn int_range(
        &mut self,
        part: &str,
        range: &RangeFile<IntProviderFile>,
    ) -> NumberRange<IntProvider> {
        match range {
            RangeFile::Point(point) => NumberRange::Point(self.condition_int(part, point)),
            RangeFile::Bounds { min, max } => NumberRange::Bounds {
                min: min
                    .as_ref()
                    .map(|min| self.condition_int(&format!("{part}.min"), min)),
                max: max
                    .as_ref()
                    .map(|max| self.condition_int(&format!("{part}.max"), max)),
            },
        }
    }

    fn float_range(
        &mut self,
        part: &str,
        range: &RangeFile<FloatProviderFile>,
    ) -> NumberRange<FloatProvider> {
        match range {
            RangeFile::Point(point) => NumberRange::Point(self.condition_float(part, point)),
            RangeFile::Bounds { min, max } => NumberRange::Bounds {
                min: min
                    .as_ref()
                    .map(|min| self.condition_float(&format!("{part}.min"), min)),
                max: max
                    .as_ref()
                    .map(|max| self.condition_float(&format!("{part}.max"), max)),
            },
        }
    }
}
