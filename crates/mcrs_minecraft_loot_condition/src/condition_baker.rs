use core::mem;
use std::vec;

use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_loot::condition::LootCondition;
use mcrs_minecraft_registries::{Registries, Registry};
use mcrs_minecraft_resource::{Id, Registered, ResourceLocation, ResourceLocationBuf, TagKey};

use crate::condition::{Condition, EnchantmentTest, ItemTest, StateSet, Test};
use crate::condition_file::{
    BlockPredicateFile, ConditionFile, ConditionForm, ItemPredicateFile, LevelsFile, OtherFields,
    PredicateTag, StateValue, Terms, TypedCondition,
};
use crate::condition_problem::ConditionProblem;
use crate::context_param::ContextParam;
use crate::float_provider_file::FloatProviderFile;
use crate::int_provider_file::IntProviderFile;
use crate::named_providers::NamedProviders;
use crate::named_source::{Dependency, NamedSource};
use crate::one_or_many::OneOrMany;

/// How deep registered predicates may name each other before the bake takes a cycle.
const PREDICATE_DEPTH: u8 = 16;

/// How many registered predicates one baker may expand, so that siblings which name each other
/// cannot multiply the work up to the depth limit.
const PREDICATE_EXPANSIONS: u32 = 4096;

const LOCATION_PARTS: &[&str] = &[
    "position",
    "biomes",
    "structures",
    "dimension",
    "smokey",
    "light",
    "fluid",
    "can_see_sky",
];

fn unsupported(other: &OtherFields, parts: &[&str]) -> bool {
    other.keys().any(|key| parts.contains(&key.as_str()))
}

/// Turns the file form of conditions and providers into their baked form, with the ids, the
/// registered predicates and the named providers of one load. Use one baker for each table.
pub struct ConditionBaker<'a> {
    definitions: &'a BlockDefinitions,
    pub(crate) registries: &'a Registries,
    predicates: Option<&'a Registry<ConditionFile>>,
    pub(crate) int_providers: Option<&'a Registry<IntProviderFile>>,
    pub(crate) float_providers: Option<&'a Registry<FloatProviderFile>>,
    pub(crate) problems: Vec<ConditionProblem>,
    depth: u8,
    expansions: u32,
    /// The parts of the provider that the bake is in, after the place that holds the provider.
    pub(crate) chain: Vec<String>,
    pub(crate) longest: usize,
    pub(crate) scope: ProviderScope,
    pub(crate) named: NamedSource<'a>,
}

/// Whether the bake is in the scope of a provider, where an inline provider is a member of the
/// reference chain (LOOT-CTX-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderScope {
    Outside,
    Inside,
}

impl<'a> ConditionBaker<'a> {
    /// A baker that resolves names against `definitions` and `registries`, and each key of a
    /// provider to its entry in `providers`, with no problem yet.
    #[must_use]
    pub fn new(
        definitions: &'a BlockDefinitions,
        registries: &'a Registries,
        providers: &'a NamedProviders,
    ) -> Self {
        Self::with_named(definitions, registries, NamedSource::Baked(providers))
    }

    /// A baker for the bake of the named providers, which records each key of a provider.
    pub(crate) fn recording(definitions: &'a BlockDefinitions, registries: &'a Registries) -> Self {
        Self::with_named(definitions, registries, NamedSource::Recording(Vec::new()))
    }

    fn with_named(
        definitions: &'a BlockDefinitions,
        registries: &'a Registries,
        named: NamedSource<'a>,
    ) -> Self {
        Self {
            definitions,
            registries,
            predicates: registries.registry::<ConditionFile>(),
            int_providers: registries.registry::<IntProviderFile>(),
            float_providers: registries.registry::<FloatProviderFile>(),
            problems: Vec::new(),
            depth: 0,
            expansions: 0,
            chain: Vec::new(),
            longest: 0,
            scope: ProviderScope::Outside,
            named,
        }
    }

    /// The most members that the providers of the bakes since the last call add to the reference
    /// chain that leads to the part that holds them (LOOT-CTX-017).
    pub fn take_longest_chain(&mut self) -> usize {
        mem::take(&mut self.longest)
    }

    pub(crate) fn take_dependencies(&mut self) -> Vec<Dependency> {
        match &mut self.named {
            NamedSource::Recording(dependencies) => mem::take(dependencies),
            NamedSource::Baked(_) => Vec::new(),
        }
    }

    /// Takes the problems of the bakes since the last call, so that the caller can tag each
    /// with the part that it baked.
    pub fn drain_problems(&mut self) -> vec::Drain<'_, ConditionProblem> {
        self.problems.drain(..)
    }

    /// Bakes `file`; a part that it cannot resolve adds a problem and makes the part fail.
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
            TypedCondition::MatchBlock(predicate) => {
                self.states(predicate).map_or(Test::Never, Test::StateIn)
            }
            TypedCondition::MatchTool { predicate: None } => Test::MatchTool(ItemTest {
                items: None,
                enchantments: Box::new([]),
            }),
            TypedCondition::MatchTool {
                predicate: Some(predicate),
            } => self
                .item_test(predicate)
                .map_or(Test::Never, Test::MatchTool),
            TypedCondition::LocationCheck {
                offset_x,
                offset_y,
                offset_z,
                predicate,
            } => {
                let offset = DVec3::new(
                    f64::from(*offset_x),
                    f64::from(*offset_y),
                    f64::from(*offset_z),
                );
                let block = match predicate {
                    // chisle: a location predicate with a part other than `block` never passes;
                    // the location view methods of those parts lift it.
                    Some(predicate) if unsupported(&predicate.other, LOCATION_PARTS) => {
                        return Test::Never;
                    }
                    Some(predicate) => predicate.block.as_ref(),
                    None => None,
                };
                match block.map(|block| self.states(block)) {
                    None => Test::BlockAt {
                        offset,
                        states: None,
                    },
                    Some(None) => Test::Never,
                    Some(Some(states)) => Test::BlockAt {
                        offset,
                        states: Some(states),
                    },
                }
            }
            TypedCondition::EntityProperties { entity, predicate } => {
                match (entity_target(entity), predicate) {
                    (None, _) => Test::Never,
                    (Some(param), None) => Test::EntityUntested(param),
                    (Some(ContextParam::ThisEntity), Some(predicate)) if predicate.is_empty() => {
                        Test::ThisEntityPresent
                    }
                    // chisle: an entity predicate with a part, or of a target other than `this`,
                    // never passes; the entity view methods and those parameters lift it.
                    (Some(_), Some(_)) => Test::Never,
                }
            }
            // chisle: each condition type without a variant here never passes; the rules of
            // those types lift it.
            TypedCondition::Other => Test::Never,
        }
    }

    /// The conditions of `terms`; `None` for an unknown tag.
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
        let file = self
            .predicates
            .and_then(|predicates| predicates.get(predicates.by_name(key.as_location())?));
        match file {
            Some(file)
                if self.depth < PREDICATE_DEPTH && self.expansions < PREDICATE_EXPANSIONS =>
            {
                self.expansions += 1;
                self.depth += 1;
                let scope = mem::replace(&mut self.scope, ProviderScope::Outside);
                let test = self.step(key.as_str(), |baker| baker.test(file));
                self.scope = scope;
                self.depth -= 1;
                test.unwrap_or(Test::Never)
            }
            _ => {
                self.problems
                    .push(ConditionProblem::UnknownPredicate(key.to_string()));
                Test::Never
            }
        }
    }

    /// The states a block predicate accepts; `None` for a predicate with a part that no rule
    /// states.
    fn states(&mut self, predicate: &BlockPredicateFile) -> Option<StateSet> {
        if unsupported(&predicate.other, &["nbt", "components", "predicates"]) {
            return None;
        }
        let blocks: Vec<Id<Block>> = match &predicate.blocks {
            Some(names) => self.ids::<Block>(names, ConditionProblem::UnknownBlock),
            None => (0..self.definitions.blocks().len())
                .filter_map(|number| Id::checked(u16::try_from(number).ok()?, usize::MAX))
                .collect(),
        };
        let mut states = Vec::new();
        'blocks: for block in blocks {
            let Some(entry) = self.definitions.entry(block) else {
                continue;
            };
            let mut wanted = Vec::new();
            for (property, value) in predicate.state.iter().flatten() {
                let StateValue::Exact(text) = value else {
                    return None;
                };
                if entry.properties.index_of(property).is_none() {
                    if predicate.blocks.is_some() {
                        self.problems.push(ConditionProblem::UnknownProperty {
                            block: entry.identifier.to_string(),
                            property: property.clone(),
                        });
                    }
                    continue 'blocks;
                }
                wanted.push((property.as_str(), text.as_str()));
            }
            states.extend((0..entry.state_count).filter_map(|offset| {
                let state = entry.state_at(offset)?;
                wanted
                    .iter()
                    .all(|(property, text)| {
                        entry
                            .value_of(state, property)
                            .is_some_and(|value| value.renders_to(text))
                    })
                    .then_some(state)
            }));
        }
        states.sort_unstable();
        Some(StateSet(states.into_boxed_slice()))
    }

    fn item_test(&mut self, predicate: &ItemPredicateFile) -> Option<ItemTest> {
        let sub = predicate.predicates.as_ref();
        if unsupported(&predicate.other, &["count", "components"])
            || sub.is_some_and(|sub| !sub.other.is_empty())
        {
            return None;
        }
        let items = predicate.items.as_ref().map(|names| {
            self.ids::<Item>(names, ConditionProblem::UnknownItem)
                .into_boxed_slice()
        });
        let mut enchantments = Vec::new();
        for test in sub
            .and_then(|sub| sub.enchantments.as_ref())
            .into_iter()
            .flatten()
        {
            let (min, max) = match test.levels {
                None => (1, u32::MAX),
                Some(LevelsFile::Exact(level)) => (level, level),
                Some(LevelsFile::Range { min, max }) => (min.unwrap_or(1), max.unwrap_or(u32::MAX)),
            };
            let any_of = test.enchantments.as_ref().map(|names| {
                self.ids::<Enchantment>(names, ConditionProblem::UnknownEnchantment)
                    .into_boxed_slice()
            });
            enchantments.push(EnchantmentTest { any_of, min, max });
        }
        Some(ItemTest {
            items,
            enchantments: enchantments.into_boxed_slice(),
        })
    }

    fn enchantment(&mut self, name: ResourceLocation<'_>) -> Option<Id<Enchantment>> {
        let id = self.lookup::<Enchantment>(name);
        if id.is_none() {
            self.problems
                .push(ConditionProblem::UnknownEnchantment(name.to_string()));
        }
        id
    }

    fn lookup<R: Registered>(&self, name: ResourceLocation<'_>) -> Option<Id<R>> {
        self.registries.entries::<R>()?.by_name(name)
    }

    /// The ids that `names` lists, each a name or a `#` tag.
    fn ids<R: Registered>(
        &mut self,
        names: &OneOrMany<String>,
        unknown: fn(String) -> ConditionProblem,
    ) -> Vec<Id<R>> {
        let mut ids = Vec::new();
        for text in names.as_slice() {
            let found = match text.strip_prefix('#') {
                Some(tag) => ResourceLocation::read(tag).ok().and_then(|tag| {
                    let tags = self.registries.tags::<R>()?;
                    let id = tags.id(TagKey::new(tag))?;
                    Some(tags.members(id).collect::<Vec<_>>())
                }),
                None => ResourceLocation::read(text)
                    .ok()
                    .and_then(|name| self.lookup::<R>(name))
                    .map(|id| vec![id]),
            };
            match found {
                Some(found) => ids.extend(found),
                None => self.problems.push(unknown(text.clone())),
            }
        }
        ids
    }
}

/// The parameter of an entity target (LOOT-CND-020); `None` for an unknown target.
pub(crate) fn entity_target(target: &str) -> Option<ContextParam> {
    match target.trim_start_matches("minecraft:") {
        "this" => Some(ContextParam::ThisEntity),
        "attacker" => Some(ContextParam::AttackingEntity),
        "direct_attacker" => Some(ContextParam::DirectAttackingEntity),
        "attacking_player" => Some(ContextParam::LastDamagePlayer),
        "target_entity" => Some(ContextParam::TargetEntity),
        "interacting_entity" => Some(ContextParam::InteractingEntity),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mcrs_minecraft_engine_registries::test_corpus;
    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use serde::de::IgnoredAny;

    use super::*;

    fn axis_y(other: &[&str]) -> Option<StateSet> {
        let blocks = &test_corpus().0.0;
        let predicate = BlockPredicateFile {
            blocks: None,
            state: Some(BTreeMap::from([(
                "axis".to_owned(),
                StateValue::Exact("y".to_owned()),
            )])),
            other: other
                .iter()
                .map(|key| ((*key).to_owned(), IgnoredAny))
                .collect(),
        };
        ConditionBaker::recording(blocks, test_registries()).states(&predicate)
    }

    #[test]
    fn a_state_test_without_blocks_skips_the_blocks_without_the_property() {
        let blocks = &test_corpus().0.0;
        let log = blocks.block("minecraft:oak_log").expect("a vanilla block");
        let y = log
            .with_text(log.default_state_id, "axis", "y")
            .expect("an axis");
        let x = log
            .with_text(log.default_state_id, "axis", "x")
            .expect("an axis");
        let states = axis_y(&["custom_note"]).expect("a state set");
        assert!(states.0.contains(&y));
        assert!(!states.0.contains(&x));
        assert!(axis_y(&["nbt"]).is_none());
    }
}
