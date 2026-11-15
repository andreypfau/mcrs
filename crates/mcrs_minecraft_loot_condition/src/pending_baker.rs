use core::num::NonZeroI64;

use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_predicate::DamageSourcePredicate;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition_baker::ConditionBaker;
use crate::condition_problem::ConditionProblem;
use crate::entity_target::EntityTarget;
use crate::float_provider::FloatProvider;
use crate::float_provider_file::FloatProviderFile;
use crate::int_provider::IntProvider;
use crate::int_provider_file::IntProviderFile;
use crate::number_range::NumberRange;
use crate::pending_test::{ObjectiveTest, PendingTest};
use crate::range_file::RangeFile;
use crate::scores_file::ScoresFile;

impl ConditionBaker<'_> {
    pub(crate) fn entity_scores(
        &mut self,
        entity: EntityTarget,
        scores: &ScoresFile,
    ) -> PendingTest {
        PendingTest::EntityScores {
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

    /// `None`, with a problem, for a predicate that does not bake.
    pub(crate) fn damage_source_properties(
        &mut self,
        predicate: Option<&DamageSourcePredicate<Named>>,
    ) -> Option<PendingTest> {
        let Some(named) = predicate else {
            return Some(PendingTest::DamageSourceProperties(None));
        };
        self.needs_smoke()?;
        let predicate = self.resolved::<DamageSourcePredicate>(Some("predicate"), named)?;
        let baked = predicate.bake(self.registries, self.definitions, &self.items);
        let baked = self.baked(baked)?;
        Some(PendingTest::DamageSourceProperties(Some(Box::new(baked))))
    }

    pub(crate) fn time_check(
        &mut self,
        clock: &ResourceLocationBuf,
        period: Option<NonZeroI64>,
        value: &RangeFile<IntProviderFile>,
    ) -> PendingTest {
        let id = self.lookup::<WorldClock>(clock.as_location());
        if id.is_none() {
            self.problems
                .push(ConditionProblem::UnknownClock(clock.to_string()));
        }
        PendingTest::TimeCheck {
            clock: id,
            period,
            value: self.int_range("value", value),
        }
    }

    pub(crate) fn int_value_check(
        &mut self,
        value: &IntProviderFile,
        test: &RangeFile<IntProviderFile>,
    ) -> PendingTest {
        PendingTest::IntValueCheck {
            value: self.condition_int("value", value),
            test: self.int_range("test", test),
        }
    }

    pub(crate) fn float_value_check(
        &mut self,
        value: &FloatProviderFile,
        test: &RangeFile<FloatProviderFile>,
    ) -> PendingTest {
        PendingTest::FloatValueCheck {
            value: self.condition_float("value", value),
            test: self.float_range("test", test),
        }
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
