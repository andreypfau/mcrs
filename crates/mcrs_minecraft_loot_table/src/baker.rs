use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::{
    Condition, ConditionBaker, ConditionFile, ContextParams, NumberFile, NumberProvider,
};
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, Registered, ResourceLocation, ResourceLocationBuf};

use crate::keyed_seed;
use crate::loot_issue::{LootIssue, LootProblem};
use crate::loot_table_file::{EntryFile, EntryKind as EntryKindFile, LootTableFile};
use crate::modifier::{Modifier, ModifierOp};
use crate::modifier_file::{LimitFile, ModifierFile, ModifierKind, ModifierObject};
use crate::table::{Entry, EntryKind, Pool, Span, Table};

/// Bakes the file form of the table `name` and adds its problems to `issues`.
pub(crate) fn bake(
    definitions: &BlockDefinitions,
    registries: &Registries,
    name: &ResourceLocationBuf,
    file: &LootTableFile,
    issues: &mut Vec<LootIssue>,
) -> Table {
    let mut baker = Baker {
        registries,
        conditions: ConditionBaker::new(definitions, registries),
        key_set: file.key_set,
        problems: Vec::new(),
        entries: Vec::new(),
    };
    let pools = file
        .pools
        .iter()
        .enumerate()
        .map(|(number, pool)| {
            let path = format!("pools[{number}]");
            Pool {
                condition: pool
                    .condition
                    .as_ref()
                    .map(|condition| baker.condition(condition, &format!("{path}.condition"))),
                rolls: baker.number(&pool.rolls, &format!("{path}.rolls")),
                bonus_rolls: pool
                    .bonus_rolls
                    .as_ref()
                    .map_or(NumberProvider::constant(0.0), |bonus_rolls| {
                        baker.number(bonus_rolls, &format!("{path}.bonus_rolls"))
                    }),
                entries: baker.entries(&pool.entries, None, &format!("{path}.entries")),
                modifiers: baker.modifiers(pool.modifier.as_ref(), &format!("{path}.modifier")),
            }
        })
        .collect();
    let modifiers = baker.modifiers(file.modifier.as_ref(), "modifier");
    let table = Table {
        name_key: keyed_seed::name_key(name.as_str()),
        pools,
        entries: baker.entries.into_boxed_slice(),
        modifiers,
    };
    issues.extend(baker.problems.into_iter().map(|(path, problem)| LootIssue {
        table: name.clone(),
        path,
        problem,
    }));
    table
}

/// Turns the file form of one table into a table of ids, and checks each part against the type
/// of the table (LOOT-CTX-013).
struct Baker<'a> {
    registries: &'a Registries,
    conditions: ConditionBaker<'a>,
    key_set: ContextKeySet,
    problems: Vec<(String, LootProblem)>,
    entries: Vec<Entry>,
}

impl Baker<'_> {
    /// Bakes `files` into consecutive places of the entry list, with their children after them.
    fn entries(&mut self, files: &[EntryFile], parent: Option<u32>, path: &str) -> Span {
        let start = index_of(self.entries.len());
        for _ in files {
            self.entries.push(Entry {
                condition: None,
                kind: EntryKind::Nothing,
                modifiers: Box::new([]),
                weight: 0,
                parent,
            });
        }
        let end = index_of(self.entries.len());
        for ((number, file), index) in files.iter().enumerate().zip(start..end) {
            let entry = self.entry(file, index, parent, &format!("{path}[{number}]"));
            let place = usize::try_from(index)
                .ok()
                .and_then(|index| self.entries.get_mut(index));
            if let Some(place) = place {
                *place = entry;
            }
        }
        Span { start, end }
    }

    fn entry(&mut self, file: &EntryFile, index: u32, parent: Option<u32>, path: &str) -> Entry {
        let condition = file
            .condition
            .as_ref()
            .map(|condition| self.condition(condition, &format!("{path}.condition")));
        let kind = match &file.kind {
            EntryKindFile::Item { name } => match self.lookup::<Item>(name.as_location()) {
                Some(item) => EntryKind::Item(Kind(item.number())),
                None => {
                    self.problems
                        .push((path.to_owned(), LootProblem::UnknownItem(name.to_string())));
                    EntryKind::Nothing
                }
            },
            // chisle: `dynamic` makes nothing, so a cracked decorated pot drops no sherds
            // (LOOT-EVL-007); a context with the block entity lifts it.
            EntryKindFile::Empty | EntryKindFile::Other => EntryKind::Nothing,
            EntryKindFile::Alternatives { children } => {
                EntryKind::Alternatives(self.children(children, index, path))
            }
            EntryKindFile::Sequence { children } => {
                EntryKind::Sequence(self.children(children, index, path))
            }
            EntryKindFile::Group { children } => {
                EntryKind::Group(self.children(children, index, path))
            }
        };
        Entry {
            condition,
            kind,
            modifiers: self.modifiers(file.modifier.as_ref(), &format!("{path}.modifier")),
            weight: file.weight,
            parent,
        }
    }

    fn children(&mut self, children: &[EntryFile], index: u32, path: &str) -> Span {
        self.entries(children, Some(index), &format!("{path}.children"))
    }

    fn modifiers(&mut self, file: Option<&ModifierFile>, path: &str) -> Box<[Modifier]> {
        match file {
            None => Box::new([]),
            Some(ModifierFile::Many(list)) => list
                .iter()
                .enumerate()
                .map(|(number, modifier)| self.modifier(modifier, &format!("{path}[{number}]")))
                .collect(),
            Some(ModifierFile::One(one)) => Box::new([self.modifier(one, path)]),
            // chisle: a registered item modifier changes nothing; no block table names one.
            Some(ModifierFile::Key(_)) => Box::new([]),
        }
    }

    fn modifier(&mut self, file: &ModifierObject, path: &str) -> Modifier {
        let condition = file
            .condition
            .as_ref()
            .map(|condition| self.condition(condition, &format!("{path}.condition")));
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
                let enchantment = self.enchantment(enchantment.as_location(), path);
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
            ModifierKind::Sequence { functions } => ModifierOp::Sequence(
                functions
                    .iter()
                    .enumerate()
                    .map(|(number, modifier)| {
                        self.modifier(modifier, &format!("{path}.functions[{number}]"))
                    })
                    .collect(),
            ),
            // chisle: a dropped stack holds an item and a count, so `copy_components`,
            // `copy_state` and the other types change nothing; a stack with components lifts it.
            ModifierKind::Other => ModifierOp::Keep,
        };
        self.take_problems(path);
        self.check(op.reads(), path);
        Modifier { condition, op }
    }

    fn condition(&mut self, file: &ConditionFile, path: &str) -> Condition {
        let condition = self.conditions.condition(file);
        self.take_problems(path);
        self.check(condition.reads(), path);
        condition
    }

    fn number(&mut self, file: &NumberFile, path: &str) -> NumberProvider {
        let number = self.conditions.number(file);
        self.take_problems(path);
        self.check(number.reads(), path);
        number
    }

    fn take_problems(&mut self, path: &str) {
        let problems = self
            .conditions
            .drain_problems()
            .map(|problem| (path.to_owned(), LootProblem::Condition(problem)));
        self.problems.extend(problems);
    }

    fn check(&mut self, reads: ContextParams, path: &str) {
        if let Err(problem) = reads.check(self.key_set) {
            self.problems
                .push((path.to_owned(), LootProblem::ParamNotAllowed(problem)));
        }
    }

    fn enchantment(&mut self, name: ResourceLocation<'_>, path: &str) -> Option<Id<Enchantment>> {
        let id = self.lookup::<Enchantment>(name);
        if id.is_none() {
            self.problems.push((
                path.to_owned(),
                LootProblem::UnknownEnchantment(name.to_string()),
            ));
        }
        id
    }

    fn lookup<R: Registered>(&self, name: ResourceLocation<'_>) -> Option<Id<R>> {
        self.registries.entries::<R>()?.by_name(name)
    }
}

fn index_of(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
