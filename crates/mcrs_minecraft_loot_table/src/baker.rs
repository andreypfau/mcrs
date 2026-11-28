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

use crate::issue_owner::IssueOwner;
use crate::keyed_seed;
use crate::loot_issue::{LootIssue, LootProblem};
use crate::loot_table_file::{
    EntryFile, EntryKind as EntryKindFile, ItemSetFile, LootTableFile, PoolFile, TableFile,
    TableSetFile, TagName,
};
use crate::modifier::Modifier;
use crate::modifier_file::ModifierFile;
use crate::table::{Entry, EntryKind, Pool, Span, Table};
use crate::table_links::{self, ChainPlace, Edge, TableLinks};

pub(crate) fn bake(
    definitions: &BlockDefinitions,
    registries: &Registries,
    entries: &NamedEntries,
    issues: &mut Vec<LootIssue>,
) -> Vec<Table> {
    let files = registries.registry::<LootTableFile>();
    let load = Load {
        definitions,
        registries,
        entries,
        files,
        modifier_files: registries.registry::<ModifierFile>(),
        first_modifier: files.map_or(0, Registry::len),
    };
    let nodes = load
        .first_modifier
        .saturating_add(load.modifier_files.map_or(0, Registry::len));
    let mut graph = Graph {
        names: Vec::with_capacity(nodes),
        tables: Vec::with_capacity(nodes),
        links: Vec::with_capacity(nodes),
    };
    let mut inline = Inline {
        first: nodes,
        tables: Vec::new(),
        links: Vec::new(),
    };
    for (id, name, file) in files.into_iter().flat_map(Registry::iter) {
        let mut baker = load.baker(file.key_set, id.index(), IssueOwner::Table, &mut inline);
        let mut baked = baker.table(file, "");
        baked.table.name_key = keyed_seed::name_key(name.as_str());
        graph.push(baker, baked, name, issues);
    }
    for (id, name, file) in load.modifier_files.into_iter().flat_map(Registry::iter) {
        let owner = load.first_modifier.saturating_add(id.index());
        let kind = IssueOwner::ItemModifier;
        let mut baker = load.baker(ContextKeySet::Generic, owner, kind, &mut inline);
        let modifiers = baker.modifiers(Some(file), "");
        let baked = mem::take(&mut baker.parts).baked(Box::new([]), modifiers);
        graph.push(baker, baked, name, issues);
    }
    let Graph {
        names,
        mut tables,
        mut links,
    } = graph;
    tables.append(&mut inline.tables);
    links.append(&mut inline.links);
    table_links::check(&links, &mut tables, &names, issues);
    tables
}

struct Load<'a> {
    definitions: &'a BlockDefinitions,
    registries: &'a Registries,
    entries: &'a NamedEntries,
    files: Option<&'a Registry<LootTableFile>>,
    modifier_files: Option<&'a Registry<ModifierFile>>,
    first_modifier: usize,
}

impl<'a> Load<'a> {
    fn baker<'i>(
        &self,
        key_set: ContextKeySet,
        owner: usize,
        kind: IssueOwner,
        inline: &'i mut Inline,
    ) -> Baker<'a, 'i> {
        Baker {
            registries: self.registries,
            files: self.files,
            modifier_files: self.modifier_files,
            first_modifier: self.first_modifier,
            conditions: ConditionBaker::new(self.definitions, self.registries, self.entries),
            key_set,
            owner,
            kind,
            problems: Vec::new(),
            parts: Parts::default(),
            inline,
        }
    }
}

struct Graph {
    names: Vec<ResourceLocationBuf>,
    tables: Vec<Table>,
    links: Vec<TableLinks>,
}

impl Graph {
    fn push(
        &mut self,
        baker: Baker<'_, '_>,
        baked: Baked,
        name: &ResourceLocationBuf,
        issues: &mut Vec<LootIssue>,
    ) {
        issues.extend(baker.problems.into_iter().map(|(path, problem)| LootIssue {
            name: name.clone(),
            owner: baker.kind,
            path,
            problem,
        }));
        self.links.push(TableLinks {
            owner: baker.owner,
            kind: baker.kind,
            checked_against: baker.key_set,
            inline: false,
            reads: baked.reads,
            edges: baked.edges,
            chains: baked.chains,
        });
        self.tables.push(baked.table);
        self.names.push(name.clone());
    }
}

struct Inline {
    first: usize,
    tables: Vec<Table>,
    links: Vec<TableLinks>,
}

#[derive(Default)]
struct Parts {
    entries: Vec<Entry>,
    items: Vec<Kind>,
    nested: Vec<Option<u32>>,
    reads: ContextParams,
    edges: Vec<Edge>,
    chains: Vec<ChainPlace>,
}

impl Parts {
    fn baked(self, pools: Box<[Pool]>, modifiers: Box<[Modifier]>) -> Baked {
        Baked {
            table: Table {
                name_key: 0,
                pools,
                entries: self.entries.into_boxed_slice(),
                items: self.items.into_boxed_slice(),
                nested: self.nested.into_boxed_slice(),
                modifiers,
            },
            reads: self.reads,
            edges: self.edges,
            chains: self.chains,
        }
    }
}

struct Baked {
    table: Table,
    reads: ContextParams,
    edges: Vec<Edge>,
    chains: Vec<ChainPlace>,
}

pub(crate) struct Baker<'a, 'i> {
    pub(crate) registries: &'a Registries,
    files: Option<&'a Registry<LootTableFile>>,
    pub(crate) modifier_files: Option<&'a Registry<ModifierFile>>,
    pub(crate) first_modifier: usize,
    pub(crate) conditions: ConditionBaker<'a>,
    key_set: ContextKeySet,
    owner: usize,
    kind: IssueOwner,
    pub(crate) problems: Vec<(String, LootProblem)>,
    parts: Parts,
    inline: &'i mut Inline,
}

impl Baker<'_, '_> {
    fn table(&mut self, file: &LootTableFile, prefix: &str) -> Baked {
        let outer = mem::take(&mut self.parts);
        let pools = file
            .pools
            .iter()
            .enumerate()
            .map(|(number, pool)| self.pool(pool, &format!("{prefix}pools[{number}]")))
            .collect();
        let modifiers = self.modifiers(file.modifier.as_ref(), &format!("{prefix}modifier"));
        mem::replace(&mut self.parts, outer).baked(pools, modifiers)
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

    pub(crate) fn entries(&mut self, files: &[EntryFile], parent: Option<u32>, path: &str) -> Span {
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

    fn children(&mut self, children: &[EntryFile], index: u32, path: &str) -> Span {
        if children.is_empty() {
            self.problems
                .push((path.to_owned(), LootProblem::NoChildren));
        }
        self.entries(children, Some(index), &format!("{path}.children"))
    }

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
                let index = self.table_key(name, path);
                self.nest(index, path);
            }
            TableFile::Inline(file) => {
                let baked = self.table(file, &format!("{path}."));
                let index = self.inline.first.saturating_add(self.inline.tables.len());
                self.inline.tables.push(baked.table);
                self.inline.links.push(TableLinks {
                    owner: self.owner,
                    kind: self.kind,
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

    pub(crate) fn table_key(&mut self, name: &ResourceLocationBuf, path: &str) -> Option<usize> {
        let id = self
            .files
            .and_then(|files| files.by_name(name.as_location()));
        if id.is_none() {
            self.problems
                .push((path.to_owned(), LootProblem::UnknownTable(name.to_string())));
        }
        id.map(Id::index)
    }

    pub(crate) fn nest(&mut self, table: Option<usize>, path: &str) -> u32 {
        let slot = self.parts.nested.len();
        let table = table.and_then(|table| u32::try_from(table).ok());
        self.parts.nested.push(table);
        if table.is_some() {
            self.parts.edges.push(Edge {
                slot,
                path: path.to_owned(),
            });
        }
        index_of(slot)
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
