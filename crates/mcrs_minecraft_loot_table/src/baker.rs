use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_loot_condition::{ConditionBaker, NumberProvider};
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, Registered, ResourceLocation};

use crate::loot_issue::LootProblem;
use crate::loot_table_file::{EntryFile, EntryKind as EntryKindFile, LootTableFile};
use crate::modifier::{Modifier, ModifierOp};
use crate::modifier_file::{LimitFile, ModifierFile, ModifierKind, ModifierObject};
use crate::table::{Entry, EntryKind, Pool, Table};

/// Turns the file form of block tables into tables of ids.
pub(crate) struct Baker<'a> {
    registries: &'a Registries,
    conditions: ConditionBaker<'a>,
    problems: Vec<LootProblem>,
}

impl<'a> Baker<'a> {
    pub(crate) fn new(definitions: &'a BlockDefinitions, registries: &'a Registries) -> Self {
        Self {
            registries,
            conditions: ConditionBaker::new(definitions, registries),
            problems: Vec::new(),
        }
    }

    /// The problems of the bakes so far.
    pub(crate) fn into_problems(self) -> impl Iterator<Item = LootProblem> {
        let conditions = self.conditions.into_problems();
        self.problems
            .into_iter()
            .chain(conditions.into_iter().map(LootProblem::Condition))
    }

    pub(crate) fn table(&mut self, file: &LootTableFile, key: u64) -> Table {
        Table {
            key,
            pools: file
                .pools
                .iter()
                .map(|pool| Pool {
                    condition: pool
                        .condition
                        .as_ref()
                        .map(|c| self.conditions.condition(c)),
                    rolls: self.conditions.number(&pool.rolls),
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
            condition: file
                .condition
                .as_ref()
                .map(|c| self.conditions.condition(c)),
            kind,
            modifiers: self.modifiers(file.modifier.as_ref()),
            weight: file.weight,
        }
    }

    fn entries(&mut self, children: &[EntryFile]) -> Box<[Entry]> {
        children.iter().map(|child| self.entry(child)).collect()
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
                count: self.conditions.number(count),
                add: *add,
            },
            ModifierKind::ExplosionDecay => ModifierOp::ExplosionDecay,
            ModifierKind::LimitCount {
                limit: LimitFile::Exact(value),
            } => {
                #[expect(clippy::cast_precision_loss, reason = "a stack count is small")]
                let value = NumberProvider::constant(*value as f32);
                ModifierOp::Limit {
                    min: Some(value.clone()),
                    max: Some(value),
                }
            }
            ModifierKind::LimitCount {
                limit: LimitFile::Range { min, max },
            } => ModifierOp::Limit {
                min: min.as_ref().map(|min| self.conditions.number(min)),
                max: max.as_ref().map(|max| self.conditions.number(max)),
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
            condition: file
                .condition
                .as_ref()
                .map(|c| self.conditions.condition(c)),
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
}
