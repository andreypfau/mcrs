use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_registries::{Registries, Registry};
use mcrs_minecraft_resource::{Id, Registered, ResourceLocation, ResourceLocationBuf, TagKey};
use mcrs_voxel::BlockPos;

use crate::condition::{Condition, EnchantmentTest, ItemTest, StateSet};
use crate::condition_file::{
    BlockPredicateFile, ConditionFile, ItemPredicateFile, LevelsFile, StateValue, TypedCondition,
};
use crate::loot_issue::LootProblem;
use crate::loot_table_file::{EntryFile, EntryKind as EntryKindFile, LootTableFile};
use crate::modifier::{Modifier, ModifierOp};
use crate::modifier_file::{LimitFile, ModifierFile, ModifierKind, ModifierObject};
use crate::number::Number;
use crate::number_file::{NumberFile, TypedNumber};
use crate::one_or_many::OneOrMany;
use crate::table::{Entry, EntryKind, Pool, Table};

/// How deep registered predicates may name each other before the bake takes a cycle.
const PREDICATE_DEPTH: u8 = 16;

/// Turns the file form of block tables into tables of ids.
pub(crate) struct Baker<'a> {
    definitions: &'a BlockDefinitions,
    registries: &'a Registries,
    predicates: Option<&'a Registry<ConditionFile>>,
    pub(crate) problems: Vec<LootProblem>,
    depth: u8,
}

impl<'a> Baker<'a> {
    pub(crate) fn new(definitions: &'a BlockDefinitions, registries: &'a Registries) -> Self {
        Self {
            definitions,
            registries,
            predicates: registries.registry::<ConditionFile>(),
            problems: Vec::new(),
            depth: 0,
        }
    }

    pub(crate) fn table(&mut self, file: &LootTableFile, key: u64) -> Table {
        Table {
            key,
            pools: file
                .pools
                .iter()
                .map(|pool| Pool {
                    condition: pool.condition.as_ref().map(|c| self.condition(c)),
                    rolls: self.number(&pool.rolls),
                    entries: pool.entries.iter().map(|e| self.entry(e)).collect(),
                    modifiers: self.modifiers(pool.modifier.as_ref()),
                })
                .collect(),
            modifiers: self.modifiers(file.modifier.as_ref()),
        }
    }

    fn entry(&mut self, file: &EntryFile) -> Entry {
        let kind = match &file.kind {
            EntryKindFile::Item { name } => match self.lookup::<Item>(name.as_location()) {
                Some(item) => EntryKind::Item(item),
                None => {
                    self.problems
                        .push(LootProblem::UnknownItem(name.to_string()));
                    EntryKind::Nothing
                }
            },
            // chisle: `dynamic` makes nothing, so a cracked decorated pot drops no sherds
            // (LOOT-EVL-007); a context with the block entity lifts it.
            EntryKindFile::Empty | EntryKindFile::Other => EntryKind::Nothing,
            EntryKindFile::Alternatives { children } => {
                EntryKind::Alternatives(self.entries(children))
            }
            EntryKindFile::Sequence { children } => EntryKind::Sequence(self.entries(children)),
            EntryKindFile::Group { children } => EntryKind::Group(self.entries(children)),
        };
        Entry {
            condition: file.condition.as_ref().map(|c| self.condition(c)),
            kind,
            modifiers: self.modifiers(file.modifier.as_ref()),
            weight: file.weight,
        }
    }

    fn entries(&mut self, children: &[EntryFile]) -> Box<[Entry]> {
        children.iter().map(|child| self.entry(child)).collect()
    }

    fn number(&mut self, file: &NumberFile) -> Number {
        match file {
            NumberFile::Constant(value) | NumberFile::Typed(TypedNumber::Constant { value }) => {
                Number::Constant(*value)
            }
            NumberFile::Typed(TypedNumber::Uniform { min, max }) => {
                Number::Uniform(Box::new(self.number(min)), Box::new(self.number(max)))
            }
            NumberFile::Typed(TypedNumber::Binomial { n, p }) => {
                Number::Binomial(Box::new(self.number(n)), Box::new(self.number(p)))
            }
            // chisle: another provider counts as 0; no block table names one.
            NumberFile::Typed(TypedNumber::Other) | NumberFile::Key(_) => Number::Constant(0.0),
        }
    }

    fn condition(&mut self, file: &ConditionFile) -> Condition {
        let typed = match file {
            ConditionFile::Key(key) => return self.predicate(key),
            ConditionFile::Inline(typed) => typed,
        };
        match &**typed {
            TypedCondition::Inverted { term } => {
                Condition::Inverted(Box::new(self.condition(term)))
            }
            TypedCondition::AnyOf { terms } => Condition::AnyOf(self.conditions(terms)),
            TypedCondition::AllOf { terms } => Condition::AllOf(self.conditions(terms)),
            TypedCondition::SurvivesExplosion => Condition::SurvivesExplosion,
            TypedCondition::RandomChance { chance } => Condition::RandomChance(self.number(chance)),
            TypedCondition::TableBonus {
                enchantment,
                chances,
            } => Condition::TableBonus {
                enchantment: self.enchantment(enchantment.as_location()),
                chances: chances.clone().into_boxed_slice(),
            },
            TypedCondition::MatchBlock(predicate) => self
                .states(predicate)
                .map_or(Condition::Never, Condition::StateIn),
            TypedCondition::MatchTool { predicate: None } => Condition::MatchTool(ItemTest {
                items: None,
                enchantments: Box::new([]),
            }),
            TypedCondition::MatchTool {
                predicate: Some(predicate),
            } => self
                .item_test(predicate)
                .map_or(Condition::Never, Condition::MatchTool),
            TypedCondition::LocationCheck {
                offset_x,
                offset_y,
                offset_z,
                predicate,
            } => {
                let offset = BlockPos::new(*offset_x, *offset_y, *offset_z);
                let block = match predicate {
                    Some(predicate) if !predicate.other.is_empty() => return Condition::Never,
                    Some(predicate) => predicate.block.as_ref(),
                    None => None,
                };
                match block.map(|block| self.states(block)) {
                    None => Condition::BlockAt {
                        offset,
                        states: None,
                    },
                    Some(None) => Condition::Never,
                    Some(Some(states)) => Condition::BlockAt {
                        offset,
                        states: Some(states),
                    },
                }
            }
            TypedCondition::EntityProperties { entity, predicate }
                if entity.trim_start_matches("minecraft:") == "this"
                    && predicate.as_ref().is_none_or(|p| p.is_empty()) =>
            {
                Condition::ThisEntity
            }
            TypedCondition::EntityProperties { .. } | TypedCondition::Other => Condition::Never,
        }
    }

    fn conditions(&mut self, terms: &OneOrMany<ConditionFile>) -> Box<[Condition]> {
        terms
            .as_slice()
            .iter()
            .map(|term| self.condition(term))
            .collect()
    }

    fn predicate(&mut self, key: &ResourceLocationBuf) -> Condition {
        let file = self
            .predicates
            .and_then(|predicates| predicates.get(predicates.by_name(key.as_location())?));
        match file {
            Some(file) if self.depth < PREDICATE_DEPTH => {
                self.depth += 1;
                let condition = self.condition(file);
                self.depth -= 1;
                condition
            }
            _ => {
                self.problems
                    .push(LootProblem::UnknownPredicate(key.to_string()));
                Condition::Never
            }
        }
    }

    /// The states a block predicate accepts; `None` for a predicate with a part that no rule
    /// states.
    fn states(&mut self, predicate: &BlockPredicateFile) -> Option<StateSet> {
        if !predicate.other.is_empty() {
            return None;
        }
        let blocks: Vec<Id<Block>> = match &predicate.blocks {
            Some(names) => self.ids::<Block>(names, LootProblem::UnknownBlock),
            None => (0..self.definitions.blocks().len())
                .filter_map(|number| Id::checked(u16::try_from(number).ok()?, usize::MAX))
                .collect(),
        };
        let mut states = Vec::new();
        for block in blocks {
            let Some(entry) = self.definitions.entry(block) else {
                continue;
            };
            let mut wanted = Vec::new();
            for (property, value) in predicate.state.iter().flatten() {
                let StateValue::Exact(text) = value else {
                    return None;
                };
                if entry.properties.index_of(property).is_none() {
                    self.problems.push(LootProblem::UnknownProperty {
                        block: entry.identifier.to_string(),
                        property: property.clone(),
                    });
                    return None;
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
        if !predicate.other.is_empty() || sub.is_some_and(|sub| !sub.other.is_empty()) {
            return None;
        }
        let items = predicate.items.as_ref().map(|names| {
            self.ids::<Item>(names, LootProblem::UnknownItem)
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
                self.ids::<Enchantment>(names, LootProblem::UnknownEnchantment)
                    .into_boxed_slice()
            });
            enchantments.push(EnchantmentTest { any_of, min, max });
        }
        Some(ItemTest {
            items,
            enchantments: enchantments.into_boxed_slice(),
        })
    }

    fn modifiers(&mut self, file: Option<&ModifierFile>) -> Box<[Modifier]> {
        match file {
            None => Box::new([]),
            Some(ModifierFile::Many(list)) => list.iter().map(|m| self.modifier(m)).collect(),
            Some(ModifierFile::One(one)) => Box::new([self.modifier(one)]),
            // chisle: a registered item modifier changes nothing; no block table names one.
            Some(ModifierFile::Key(_)) => Box::new([]),
        }
    }

    fn modifier(&mut self, file: &ModifierObject) -> Modifier {
        let op = match &file.kind {
            ModifierKind::SetCount { count, add } => ModifierOp::SetCount {
                count: self.number(count),
                add: *add,
            },
            ModifierKind::ExplosionDecay => ModifierOp::ExplosionDecay,
            ModifierKind::LimitCount {
                limit: LimitFile::Exact(value),
            } => {
                #[expect(clippy::cast_precision_loss, reason = "a stack count is small")]
                let value = Number::Constant(*value as f32);
                ModifierOp::Limit {
                    min: Some(value.clone()),
                    max: Some(value),
                }
            }
            ModifierKind::LimitCount {
                limit: LimitFile::Range { min, max },
            } => ModifierOp::Limit {
                min: min.as_ref().map(|min| self.number(min)),
                max: max.as_ref().map(|max| self.number(max)),
            },
            ModifierKind::ApplyBonus {
                enchantment,
                formula,
                parameters,
            } => {
                let enchantment = self.enchantment(enchantment.as_location());
                match formula.as_str() {
                    "minecraft:ore_drops" => ModifierOp::OreDrops(enchantment),
                    "minecraft:uniform_bonus_count" => ModifierOp::UniformBonus {
                        enchantment,
                        multiplier: parameters.bonus_multiplier.unwrap_or(0),
                    },
                    "minecraft:binomial_with_bonus_count" => ModifierOp::BinomialBonus {
                        enchantment,
                        extra: parameters.extra.unwrap_or(0),
                        probability: parameters.probability.unwrap_or(0.0),
                    },
                    _ => ModifierOp::Keep,
                }
            }
            ModifierKind::Sequence { functions } => {
                ModifierOp::Sequence(functions.iter().map(|m| self.modifier(m)).collect())
            }
            // chisle: a dropped stack holds an item and a count, so `copy_components`,
            // `copy_state` and the other types change nothing; a stack with components lifts it.
            ModifierKind::Other => ModifierOp::Keep,
        };
        Modifier {
            condition: file.condition.as_ref().map(|c| self.condition(c)),
            op,
        }
    }

    fn enchantment(&mut self, name: ResourceLocation<'_>) -> Option<Id<Enchantment>> {
        let id = self.lookup::<Enchantment>(name);
        if id.is_none() {
            self.problems
                .push(LootProblem::UnknownEnchantment(name.to_string()));
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
        unknown: fn(String) -> LootProblem,
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
