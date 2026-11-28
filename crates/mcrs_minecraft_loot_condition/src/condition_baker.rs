use core::mem;
use std::vec;

use glam::DVec3;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item::component::predicate::{BlockPredicate, ItemPredicate};
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_loot::condition::LootCondition;
use mcrs_minecraft_predicate::{
    BakedBlockPredicate, BakedEntityPredicate, BakedItemPredicate, BakedLocationPredicate,
    EntityPredicate, LocationPredicate, PredicateIssue, SmokeTable,
};
use mcrs_minecraft_registries::{Named, Registries, Registry, Resolve};
use mcrs_minecraft_resource::{Id, Registered, ResourceLocation, ResourceLocationBuf};

use crate::condition::{Condition, Test};
use crate::condition_file::{ConditionFile, ConditionForm, PredicateTag, Terms, TypedCondition};
use crate::condition_problem::ConditionProblem;
use crate::context_param::ContextParam;
use crate::context_params::ContextParams;
use crate::entity_target::EntityTarget;
use crate::entry_kind::EntryKind;
use crate::float_provider_file::FloatProviderFile;
use crate::int_provider_file::IntProviderFile;
use crate::named_entries::NamedEntries;
use crate::named_source::{Dependency, NamedSource};

pub struct ConditionBaker<'a> {
    pub(crate) definitions: &'a BlockDefinitions,
    pub(crate) registries: &'a Registries,
    pub(crate) predicates: Option<&'a Registry<ConditionFile>>,
    pub(crate) int_providers: Option<&'a Registry<IntProviderFile>>,
    pub(crate) float_providers: Option<&'a Registry<FloatProviderFile>>,
    pub(crate) items: ItemTable,
    smoke: Option<PredicateIssue>,
    pub(crate) problems: Vec<ConditionProblem>,
    pub(crate) chain: Vec<String>,
    pub(crate) longest: usize,
    pub(crate) scope: ProviderScope,
    pub(crate) named: NamedSource<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderScope {
    Outside,
    Inside,
}

impl<'a> ConditionBaker<'a> {
    #[must_use]
    pub fn new(
        definitions: &'a BlockDefinitions,
        registries: &'a Registries,
        entries: &'a NamedEntries,
    ) -> Self {
        let smoke = entries.smoke.as_ref().err().copied();
        let named = NamedSource::Baked(entries);
        Self::with_named(definitions, registries, entries.items, smoke, named)
    }

    pub(crate) fn recording(
        definitions: &'a BlockDefinitions,
        registries: &'a Registries,
        items: ItemTable,
        smoke: Result<&SmokeTable, &PredicateIssue>,
    ) -> Self {
        let smoke = smoke.err().copied();
        let named = NamedSource::Recording(Vec::new());
        Self::with_named(definitions, registries, items, smoke, named)
    }

    fn with_named(
        definitions: &'a BlockDefinitions,
        registries: &'a Registries,
        items: ItemTable,
        smoke: Option<PredicateIssue>,
        named: NamedSource<'a>,
    ) -> Self {
        Self {
            definitions,
            registries,
            predicates: registries.registry::<ConditionFile>(),
            int_providers: registries.registry::<IntProviderFile>(),
            float_providers: registries.registry::<FloatProviderFile>(),
            items,
            smoke,
            problems: Vec::new(),
            chain: Vec::new(),
            longest: 0,
            scope: ProviderScope::Outside,
            named,
        }
    }

    pub fn take_longest_chain(&mut self) -> usize {
        mem::take(&mut self.longest)
    }

    pub(crate) fn take_dependencies(&mut self) -> Vec<Dependency> {
        match &mut self.named {
            NamedSource::Recording(dependencies) => mem::take(dependencies),
            NamedSource::Baked(_) => Vec::new(),
        }
    }

    pub fn drain_problems(&mut self) -> vec::Drain<'_, ConditionProblem> {
        self.problems.drain(..)
    }

    pub fn condition(&mut self, file: &ConditionFile) -> Condition {
        Condition(self.test(file))
    }

    pub(crate) fn test(&mut self, file: &ConditionFile) -> Test {
        let typed = match &file.0 {
            ConditionForm::Key(key) => return self.predicate(key),
            ConditionForm::Inline(typed) => typed,
        };
        match &**typed {
            TypedCondition::Inverted { term } => Test::Inverted(Box::new(self.test(term))),
            TypedCondition::AnyOf { terms } => self.terms(terms).map_or(Test::Never, Test::AnyOf),
            TypedCondition::AllOf { terms } => self.terms(terms).map_or(Test::Never, Test::AllOf),
            TypedCondition::SurvivesExplosion => Test::SurvivesExplosion,
            TypedCondition::RandomChance { chance } => {
                Test::RandomChance(self.condition_float("chance", chance))
            }
            TypedCondition::TableBonus {
                enchantment,
                chances,
            } => Test::TableBonus {
                enchantment: self.enchantment(enchantment.as_location()),
                chances: chances.clone().into_boxed_slice(),
            },
            TypedCondition::MatchBlock(predicate) => self.match_block(predicate),
            TypedCondition::MatchTool { predicate } => {
                match predicate
                    .as_ref()
                    .map(|predicate| self.item_predicate(predicate))
                {
                    None => Test::MatchTool(None),
                    Some(None) => Test::Never,
                    Some(Some(predicate)) => Test::MatchTool(Some(Box::new(predicate))),
                }
            }
            TypedCondition::LocationCheck {
                offset_x,
                offset_y,
                offset_z,
                predicate,
            } => {
                let offset =
                    [offset_x, offset_y, offset_z].map(|offset| f64::from(offset.unwrap_or(0)));
                match predicate
                    .as_ref()
                    .map(|predicate| self.location_predicate(predicate))
                {
                    Some(None) => Test::Never,
                    predicate => Test::LocationCheck {
                        offset: DVec3::from_array(offset),
                        predicate: predicate.flatten().map(Box::new),
                    },
                }
            }
            TypedCondition::EntityProperties { entity, predicate } => {
                self.entity_properties(*entity, predicate.as_deref())
            }
            TypedCondition::RandomChanceWithEnchantedBonus {
                enchantment,
                unenchanted_chance,
                enchanted_chance,
            } => self.enchanted_bonus(
                enchantment.as_location(),
                *unenchanted_chance,
                enchanted_chance,
            ),
            TypedCondition::KilledByPlayer => Test::KilledByPlayer,
            TypedCondition::EntityScores { entity, scores } => self.entity_scores(*entity, scores),
            TypedCondition::DamageSourceProperties { predicate } => self
                .damage_source_properties(predicate.as_deref())
                .unwrap_or(Test::Never),
            TypedCondition::WeatherCheck {
                raining,
                thundering,
            } => Test::WeatherCheck {
                raining: *raining,
                thundering: *thundering,
            },
            TypedCondition::TimeCheck {
                clock,
                period,
                value,
            } => self.time_check(clock, *period, value),
            TypedCondition::IntValueCheck { value, test } => self.int_value_check(value, test),
            TypedCondition::FloatValueCheck { value, test } => self.float_value_check(value, test),
            TypedCondition::EnchantmentActiveCheck { active } => {
                Test::EnchantmentActiveCheck { active: *active }
            }
            TypedCondition::EnvironmentAttributeCheck(check) => {
                Test::EnvironmentAttributeCheck(Box::new(check.clone()))
            }
        }
    }

    fn terms(&mut self, terms: &Terms) -> Option<Box<[Test]>> {
        let tag = match terms {
            Terms::List(terms) => {
                return Some(terms.as_slice().iter().map(|t| self.test(t)).collect());
            }
            Terms::Tag(PredicateTag(tag)) => tag,
        };
        let registries = self.registries;
        let members = registries.tags::<LootCondition>().and_then(|tags| {
            let names = registries.entries::<LootCondition>()?;
            let members = tags.members(tags.id(tag.as_key())?);
            members.map(|id| names.name(id)).collect::<Option<Vec<_>>>()
        });
        let Some(members) = members else {
            self.problems
                .push(ConditionProblem::UnknownPredicate(format!(
                    "#{}",
                    tag.as_str()
                )));
            return None;
        };
        Some(members.into_iter().map(|key| self.predicate(key)).collect())
    }

    fn predicate(&mut self, key: &ResourceLocationBuf) -> Test {
        let registry = self.predicates;
        self.named(key, registry, EntryKind::Predicate, |index, reads| {
            Test::Named { index, reads }
        })
        .unwrap_or(Test::Never)
    }

    fn match_block(&mut self, named: &BlockPredicate<Named>) -> Test {
        let Some(predicate) = self.resolved::<BlockPredicate>(None, named) else {
            return Test::Never;
        };
        let reads = match predicate.nbt.is_some() || !predicate.matchers.is_empty() {
            true => ContextParams::of(&[ContextParam::BlockState, ContextParam::BlockEntity]),
            false => ContextParams::of(&[ContextParam::BlockState]),
        };
        let baked = BakedBlockPredicate::bake(&predicate, self.registries, self.definitions);
        match self.baked(baked) {
            Some(predicate) => Test::MatchBlock {
                predicate: Box::new(predicate),
                reads,
            },
            None => Test::Never,
        }
    }

    pub fn item_predicate(&mut self, named: &ItemPredicate<Named>) -> Option<BakedItemPredicate> {
        let predicate = self.resolved::<ItemPredicate>(Some("predicate"), named)?;
        let baked = BakedItemPredicate::bake(&predicate, self.registries, &self.items);
        self.baked(baked)
    }

    fn location_predicate(
        &mut self,
        named: &LocationPredicate<Named>,
    ) -> Option<BakedLocationPredicate> {
        self.needs_smoke()?;
        let predicate = self.resolved::<LocationPredicate>(Some("predicate"), named)?;
        let baked = predicate.bake(self.registries, self.definitions);
        self.baked(baked)
    }

    fn entity_properties(
        &mut self,
        entity: EntityTarget,
        predicate: Option<&EntityPredicate<Named>>,
    ) -> Test {
        match predicate.map(|predicate| self.entity_predicate(predicate)) {
            Some(None) => Test::Never,
            predicate => Test::EntityProperties {
                target: entity.param(),
                predicate: predicate.flatten().map(Box::new),
            },
        }
    }

    fn entity_predicate(&mut self, named: &EntityPredicate<Named>) -> Option<BakedEntityPredicate> {
        self.needs_smoke()?;
        let predicate = self.resolved::<EntityPredicate>(Some("predicate"), named)?;
        let baked = predicate.bake(self.registries, self.definitions, &self.items);
        self.baked(baked)
    }

    pub(crate) fn needs_smoke(&mut self) -> Option<()> {
        match self.smoke {
            None => Some(()),
            Some(issue) => {
                self.problems.push(ConditionProblem::Predicate(issue));
                None
            }
        }
    }

    pub(crate) fn resolved<T: Resolve>(
        &mut self,
        field: Option<&str>,
        named: &T::Named,
    ) -> Option<T>
    where
        T::Named: Clone,
    {
        match T::resolve(named.clone(), self.registries) {
            Ok(resolved) => Some(resolved),
            Err(error) => {
                let path = match (field, error.path()) {
                    (Some(field), Some(path)) => format!("{field}.{path}"),
                    (Some(field), None) => field.to_owned(),
                    (None, path) => path.unwrap_or_default().to_owned(),
                };
                self.problems.push(ConditionProblem::Unresolved {
                    path,
                    message: error.message().to_owned(),
                });
                None
            }
        }
    }

    pub(crate) fn baked<B>(&mut self, baked: Result<B, PredicateIssue>) -> Option<B> {
        baked
            .map_err(|issue| self.problems.push(ConditionProblem::Predicate(issue)))
            .ok()
    }

    pub(crate) fn enchantment(&mut self, name: ResourceLocation<'_>) -> Option<Id<Enchantment>> {
        let id = self.lookup::<Enchantment>(name);
        if id.is_none() {
            self.problems
                .push(ConditionProblem::UnknownEnchantment(name.to_string()));
        }
        id
    }

    pub(crate) fn lookup<R: Registered>(&self, name: ResourceLocation<'_>) -> Option<Id<R>> {
        self.registries.entries::<R>()?.by_name(name)
    }
}
