use core::mem;

use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot::loot::LootTable;
use mcrs_minecraft_loot_condition::{
    Condition, ConditionBaker, ConditionFile, ContextParams, DynamicName, FloatProvider,
    FloatProviderFile, IntProvider, IntProviderFile, NamedEntries,
};
use mcrs_minecraft_registries::{Registries, Registry};
use mcrs_minecraft_resource::{Id, Registered, ResourceLocation, ResourceLocationBuf, TagKey};

use crate::keyed_seed;
use crate::loot_issue::{LootIssue, LootProblem};
use crate::loot_table_file::{
    EntryFile, EntryKind as EntryKindFile, ItemSetFile, LootTableFile, PoolFile, TableFile,
    TableSetFile, TagName,
};
use crate::table::{Entry, EntryKind, Pool, Span, Table};
use crate::table_links::{self, ChainPlace, Edge, TableLinks};

/// Bakes each table of the registry `loot_table` of `registries`, at the index of its id, and
/// after them each inline table, and adds the problems of the load to `issues`.
pub(crate) fn bake(
    definitions: &BlockDefinitions,
    registries: &Registries,
    entries: &NamedEntries,
    issues: &mut Vec<LootIssue>,
) -> Vec<Table> {
    let files = registries.registry::<LootTableFile>();
    let named = files.map_or(0, Registry::len);
    let mut names = Vec::with_capacity(named);
    let mut tables = Vec::with_capacity(named);
    let mut links = Vec::with_capacity(named);
    let mut inline = Inline {
        first: named,
        tables: Vec::new(),
        links: Vec::new(),
    };
    for (id, name, file) in files.into_iter().flat_map(Registry::iter) {
        let mut baker = Baker {
            registries,
            files,
            conditions: ConditionBaker::new(definitions, registries, entries),
            key_set: file.key_set,
            owner: id.index(),
            problems: Vec::new(),
            parts: Parts::default(),
            inline: &mut inline,
        };
        let mut baked = baker.table(file, "");
        baked.table.name_key = keyed_seed::name_key(name.as_str());
        issues.extend(baker.problems.into_iter().map(|(path, problem)| LootIssue {
            table: name.clone(),
            path,
            problem,
        }));
        links.push(TableLinks {
            owner: id.index(),
            checked_against: file.key_set,
            inline: false,
            reads: baked.reads,
            edges: baked.edges,
            chains: baked.chains,
        });
        tables.push(baked.table);
        names.push(name.clone());
    }
    tables.append(&mut inline.tables);
    links.append(&mut inline.links);
    table_links::check(&links, &mut tables, &names, issues);
    tables
}

/// The inline tables of the load, which sit after the tables of the registry.
struct Inline {
    first: usize,
    tables: Vec<Table>,
    links: Vec<TableLinks>,
}

/// The lists of the table that the bake makes.
#[derive(Default)]
struct Parts {
    entries: Vec<Entry>,
    items: Vec<Kind>,
    nested: Vec<Option<u32>>,
    reads: ContextParams,
    edges: Vec<Edge>,
    chains: Vec<ChainPlace>,
}

struct Baked {
    table: Table,
    reads: ContextParams,
    edges: Vec<Edge>,
    chains: Vec<ChainPlace>,
}

/// Turns the file form of one table of the registry and of its inline tables into tables of
/// ids, and checks each part against the type of the table (LOOT-CTX-013).
pub(crate) struct Baker<'a, 'i> {
    pub(crate) registries: &'a Registries,
    files: Option<&'a Registry<LootTableFile>>,
    pub(crate) conditions: ConditionBaker<'a>,
    key_set: ContextKeySet,
    owner: usize,
    pub(crate) problems: Vec<(String, LootProblem)>,
    parts: Parts,
    inline: &'i mut Inline,
}

impl Baker<'_, '_> {
    /// Bakes `file` with each path after `prefix`.
    fn table(&mut self, file: &LootTableFile, prefix: &str) -> Baked {
        let outer = mem::take(&mut self.parts);
        let pools = file
            .pools
            .iter()
            .enumerate()
            .map(|(number, pool)| self.pool(pool, &format!("{prefix}pools[{number}]")))
            .collect();
        let modifiers = self.modifiers(file.modifier.as_ref(), &format!("{prefix}modifier"));
        let Parts {
            entries,
            items,
            nested,
            reads,
            edges,
            chains,
        } = mem::replace(&mut self.parts, outer);
        let table = Table {
            name_key: 0,
            pools,
            entries: entries.into_boxed_slice(),
            items: items.into_boxed_slice(),
            nested: nested.into_boxed_slice(),
            modifiers,
        };
        Baked {
            table,
            reads,
            edges,
            chains,
        }
    }

    fn pool(&mut self, pool: &PoolFile, path: &str) -> Pool {
        Pool {
            condition: pool
                .condition
                .as_ref()
                .map(|condition| self.condition(condition, &format!("{path}.condition"))),
            rolls: self.int_provider(&pool.rolls, &format!("{path}.rolls")),
            bonus_rolls: pool
                .bonus_rolls
                .as_ref()
                .map_or(FloatProvider::constant(0.0), |bonus_rolls| {
                    self.float_provider(bonus_rolls, &format!("{path}.bonus_rolls"))
                }),
            entries: self.entries(&pool.entries, None, &format!("{path}.entries")),
            modifiers: self.modifiers(pool.modifier.as_ref(), &format!("{path}.modifier")),
        }
    }

    /// Bakes `files` into consecutive places of the entry list, with their children after them.
    fn entries(&mut self, files: &[EntryFile], parent: Option<u32>, path: &str) -> Span {
        let start = index_of(self.parts.entries.len());
        for _ in files {
            self.parts.entries.push(Entry {
                condition: None,
                kind: EntryKind::Empty,
                modifiers: Box::new([]),
                weight: 0,
                quality: 0,
                parent,
            });
        }
        let end = index_of(self.parts.entries.len());
        for ((number, file), index) in files.iter().enumerate().zip(start..end) {
            let entry = self.entry(file, index, parent, &format!("{path}[{number}]"));
            let place = usize::try_from(index)
                .ok()
                .and_then(|index| self.parts.entries.get_mut(index));
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
                    EntryKind::Empty
                }
            },
            EntryKindFile::Tag { items, expand } => {
                let members = self.items(items, path);
                if *expand {
                    EntryKind::ExpandedTag(members)
                } else {
                    EntryKind::Tag(members)
                }
            }
            EntryKindFile::LootTable { value, expand } => {
                let tables = self.tables(value, path);
                if *expand {
                    EntryKind::ExpandedTables(tables)
                } else {
                    EntryKind::Tables(tables)
                }
            }
            EntryKindFile::Dynamic { name } => DynamicName::from_location(name.as_location())
                .map_or(EntryKind::Empty, EntryKind::Dynamic),
            // chisle: `slots` makes nothing, checks no field of a typed slot source and resolves no
            // key until the item predicate, container components and `slot_source` registry exist.
            EntryKindFile::Empty | EntryKindFile::Slots { .. } => EntryKind::Empty,
            EntryKindFile::Alternatives { children } => {
                let last = children.len().saturating_sub(1);
                for (number, child) in children.iter().enumerate().take(last) {
                    if child.condition.is_none() {
                        self.problems.push((
                            format!("{path}.children[{number}]"),
                            LootProblem::AlternativeWithoutCondition,
                        ));
                    }
                }
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
            quality: file.quality,
            parent,
        }
    }

    /// LOOT-CTX-016: a composite entry without children is a problem.
    fn children(&mut self, children: &[EntryFile], index: u32, path: &str) -> Span {
        if children.is_empty() {
            self.problems
                .push((path.to_owned(), LootProblem::NoChildren));
        }
        self.entries(children, Some(index), &format!("{path}.children"))
    }

    /// Adds the items of a `tag` entry to the item list, in the member order of a tag and the
    /// order of a list (LOOT-EVL-009).
    fn items(&mut self, set: &ItemSetFile, path: &str) -> Span {
        let start = index_of(self.parts.items.len());
        let path = format!("{path}.items");
        match set {
            ItemSetFile::Tag(TagName(name)) => {
                let tags = self.registries.tags::<Item>();
                let tag = tags.as_ref().and_then(|tags| {
                    let tag = tags.id(TagKey::new(name.as_location()))?;
                    Some(tags.members(tag).map(|item| Kind(item.number())))
                });
                match tag {
                    Some(members) => self.parts.items.extend(members),
                    None => self
                        .problems
                        .push((path, LootProblem::UnknownTag(name.to_string()))),
                }
            }
            ItemSetFile::Many(names) => {
                for name in names {
                    self.item(name, &path);
                }
            }
            ItemSetFile::One(name) => self.item(name, &path),
        }
        Span {
            start,
            end: index_of(self.parts.items.len()),
        }
    }

    fn item(&mut self, name: &ResourceLocationBuf, path: &str) {
        match self.lookup::<Item>(name.as_location()) {
            Some(item) => self.parts.items.push(Kind(item.number())),
            None => self
                .problems
                .push((path.to_owned(), LootProblem::UnknownItem(name.to_string()))),
        }
    }

    /// Adds the tables of a `loot_table` entry to the list of nested tables, in the member order
    /// of a tag and the order of a list (LOOT-EVL-010).
    fn tables(&mut self, set: &TableSetFile, path: &str) -> Span {
        let start = index_of(self.parts.nested.len());
        let path = format!("{path}.value");
        match set {
            TableSetFile::Tag(TagName(name)) => {
                let tags = self.registries.tags::<LootTable>();
                let tag: Option<Vec<usize>> = tags.as_ref().and_then(|tags| {
                    let tag = tags.id(TagKey::new(name.as_location()))?;
                    Some(tags.members(tag).map(Id::index).collect())
                });
                match tag {
                    Some(members) => {
                        for member in members {
                            self.nest(Some(member), &path);
                        }
                    }
                    None => self
                        .problems
                        .push((path, LootProblem::UnknownTag(name.to_string()))),
                }
            }
            TableSetFile::Many(elements) => {
                for (number, element) in elements.iter().enumerate() {
                    self.table_element(element, &format!("{path}[{number}]"));
                }
            }
            TableSetFile::One(element) => self.table_element(element, &path),
        }
        Span {
            start,
            end: index_of(self.parts.nested.len()),
        }
    }

    fn table_element(&mut self, element: &TableFile, path: &str) {
        match element {
            TableFile::Key(name) => {
                let id = self
                    .files
                    .and_then(|files| files.by_name(name.as_location()));
                if id.is_none() {
                    self.problems
                        .push((path.to_owned(), LootProblem::UnknownTable(name.to_string())));
                }
                self.nest(id.map(Id::index), path);
            }
            TableFile::Inline(file) => {
                let baked = self.table(file, &format!("{path}."));
                let index = self.inline.first.saturating_add(self.inline.tables.len());
                self.inline.tables.push(baked.table);
                self.inline.links.push(TableLinks {
                    owner: self.owner,
                    checked_against: self.key_set,
                    inline: true,
                    reads: baked.reads,
                    edges: baked.edges,
                    chains: baked.chains,
                });
                self.nest(Some(index), path);
            }
        }
    }

    fn nest(&mut self, table: Option<usize>, path: &str) {
        let slot = self.parts.nested.len();
        let table = table.and_then(|table| u32::try_from(table).ok());
        self.parts.nested.push(table);
        if table.is_some() {
            self.parts.edges.push(Edge {
                slot,
                path: path.to_owned(),
            });
        }
    }

    pub(crate) fn condition(&mut self, file: &ConditionFile, path: &str) -> Condition {
        let condition = self.conditions.condition(file);
        self.take_problems(path);
        self.check(condition.reads(), path);
        condition
    }

    fn int_provider(&mut self, file: &IntProviderFile, path: &str) -> IntProvider {
        let provider = self.conditions.int_provider(file);
        self.take_problems(path);
        self.check(provider.reads(), path);
        provider
    }

    fn float_provider(&mut self, file: &FloatProviderFile, path: &str) -> FloatProvider {
        let provider = self.conditions.float_provider(file);
        self.take_problems(path);
        self.check(provider.reads(), path);
        provider
    }

    pub(crate) fn take_problems(&mut self, path: &str) {
        let problems = self
            .conditions
            .drain_problems()
            .map(|problem| (path.to_owned(), LootProblem::Condition(problem)));
        self.problems.extend(problems);
    }

    pub(crate) fn check(&mut self, reads: ContextParams, path: &str) {
        self.parts.reads = self.parts.reads.union(reads);
        let length = self.conditions.take_longest_chain();
        if length > 0 {
            self.parts.chains.push(ChainPlace {
                path: path.to_owned(),
                length,
            });
        }
        if let Err(problem) = reads.check(self.key_set) {
            self.problems
                .push((path.to_owned(), LootProblem::ParamNotAllowed(problem)));
        }
    }

    pub(crate) fn lookup<R: Registered>(&self, name: ResourceLocation<'_>) -> Option<Id<R>> {
        self.registries.entries::<R>()?.by_name(name)
    }
}

fn index_of(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
