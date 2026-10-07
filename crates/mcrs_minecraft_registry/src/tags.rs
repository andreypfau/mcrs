use crate::id::Id;
use crate::names::NameTable;
use crate::registry::Registry;
use crate::set::{self, ScopeError};
use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

type Name = ResourceLocation<Arc<str>>;

pub struct TagId<R> {
    number: u16,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for TagId<R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<R> Copy for TagId<R> {}
impl<R> PartialEq for TagId<R> {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number
    }
}
impl<R> Eq for TagId<R> {}
impl<R> std::hash::Hash for TagId<R> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.number.hash(state);
    }
}
impl<R> fmt::Debug for TagId<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TagId({})", self.number)
    }
}

impl<R> TagId<R> {
    fn from_number(number: u16) -> Self {
        TagId {
            number,
            _marker: PhantomData,
        }
    }

    pub fn number(self) -> u16 {
        self.number
    }
}

#[derive(Debug)]
pub struct TagTable {
    registry: Name,
    names: Box<[Name]>,
    numbers: HashMap<Name, u16>,
    members: Box<[Box<[u16]>]>,
    bits: Box<[FixedBitSet]>,
}

impl TagTable {
    pub fn registry(&self) -> &Name {
        &self.registry
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn names(&self) -> &[Name] {
        &self.names
    }

    pub fn members(&self, tag: usize) -> &[u16] {
        &self.members[tag]
    }
}

pub struct Tags<R> {
    table: Arc<TagTable>,
    _marker: PhantomData<fn() -> R>,
}

impl<R> Clone for Tags<R> {
    fn clone(&self) -> Self {
        Tags {
            table: Arc::clone(&self.table),
            _marker: PhantomData,
        }
    }
}

impl<R> fmt::Debug for Tags<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tags")
            .field("key", &self.table.registry)
            .field("len", &self.table.len())
            .finish()
    }
}

impl<R> Tags<R> {
    pub fn new(table: Arc<TagTable>) -> Self {
        Tags {
            table,
            _marker: PhantomData,
        }
    }

    pub fn from_members(registry: &Registry<R>, tags: Vec<(Name, Vec<Id<R>>)>) -> Self {
        let mut seen = HashSet::with_capacity(tags.len());
        let tags: Vec<_> = tags
            .into_iter()
            .filter(|(tag, _)| seen.insert(tag.clone()))
            .map(|(tag, members)| {
                let members = members.into_iter().map(Id::number).collect();
                (tag, members)
            })
            .collect();
        Tags::new(Arc::new(assemble(registry.table(), tags.into_iter())))
    }

    pub fn get<S: AsRef<str>>(&self, key: &TagKey<R, S>) -> Option<TagId<R>> {
        self.table
            .numbers
            .get(key.as_str())
            .copied()
            .map(TagId::from_number)
    }

    pub fn registry(&self) -> &Name {
        &self.table.registry
    }
}

impl<R: 'static> Tags<R> {
    pub fn in_scope<T>(
        parsing: &'static str,
        run: impl FnOnce(&Tags<R>) -> T,
    ) -> Result<T, ScopeError> {
        set::in_scope::<R, _, _>(parsing, |set| set.tags::<R>(), run)
    }
}

impl<R> Tags<R> {
    pub fn table(&self) -> &Arc<TagTable> {
        &self.table
    }

    pub fn tag_ids(&self) -> impl ExactSizeIterator<Item = TagId<R>> + use<R> {
        (0..=u16::MAX)
            .take(self.table.names.len())
            .map(TagId::from_number)
    }

    pub fn members(
        &self,
        tag: TagId<R>,
    ) -> impl ExactSizeIterator<Item = Id<R>> + DoubleEndedIterator + '_ {
        self.table.members[usize::from(tag.number)]
            .iter()
            .map(|&number| Id::from_number(number))
    }

    pub fn contains(&self, tag: TagId<R>, id: Id<R>) -> bool {
        self.table.bits[usize::from(tag.number)].contains(id.index())
    }

    pub fn name(&self, tag: TagId<R>) -> &Name {
        &self.table.names[usize::from(tag.number)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagRules {
    Static,
    World,
}

#[derive(Clone, Copy, Debug)]
pub struct TagSource<'a> {
    pub pack: &'a str,
    pub path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagProblem {
    Error {
        registry: Name,
        tag: Name,
        file: String,
        message: String,
    },
    Dropped {
        registry: Name,
        tag: Name,
        message: String,
    },
    Skipped {
        registry: Name,
        tag: Name,
        file: String,
        message: String,
    },
}

impl fmt::Display for TagProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TagProblem::Error {
                registry,
                tag,
                file,
                message,
            } => write!(f, "registry {registry} tag #{tag} ({file}): {message}"),
            TagProblem::Dropped {
                registry,
                tag,
                message,
            } => write!(f, "registry {registry} tag #{tag} dropped: {message}"),
            TagProblem::Skipped {
                registry,
                tag,
                file,
                message,
            } => write!(
                f,
                "registry {registry} tag #{tag}: skipped {file}: {message}"
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagEntry {
    pub id: Name,
    pub tag: bool,
    pub required: bool,
}

impl fmt::Display for TagEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tag {
            f.write_str("#")?;
        }
        write!(f, "{}", self.id)?;
        if !self.required {
            f.write_str("?")?;
        }
        Ok(())
    }
}

fn read_reference(text: &str) -> Result<(Name, bool), String> {
    let (tag, text) = match text.strip_prefix('#') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    ResourceLocation::read(text)
        .map(|id| (id, tag))
        .map_err(|error| error.to_string())
}

fn write_reference(id: &Name, tag: bool) -> String {
    if tag {
        format!("#{id}")
    } else {
        id.to_string()
    }
}

impl<'de> Deserialize<'de> for TagEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntryVisitor;

        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = TagEntry;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an element or #tag name, or an object with `id` and `required`")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<TagEntry, E> {
                let (id, tag) = read_reference(text).map_err(E::custom)?;
                Ok(TagEntry {
                    id,
                    tag,
                    required: true,
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<TagEntry, A::Error> {
                #[derive(Deserialize)]
                struct Object {
                    id: String,
                    #[serde(default = "required_by_default")]
                    required: bool,
                }

                fn required_by_default() -> bool {
                    true
                }

                let object = Object::deserialize(MapAccessDeserializer::new(map))?;
                let (id, tag) = read_reference(&object.id).map_err(de::Error::custom)?;
                Ok(TagEntry {
                    id,
                    tag,
                    required: object.required,
                })
            }
        }

        deserializer.deserialize_any(EntryVisitor)
    }
}

impl Serialize for TagEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let reference = write_reference(&self.id, self.tag);
        if self.required {
            serializer.serialize_str(&reference)
        } else {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("id", &reference)?;
            map.serialize_entry("required", &false)?;
            map.end()
        }
    }
}

#[derive(Deserialize)]
struct TagFile {
    values: Vec<TagEntry>,
    #[serde(default)]
    replace: bool,
}

pub fn number_tags(current: &BTreeSet<Name>, prior: Option<&[Name]>) -> Vec<Name> {
    let prior = prior.unwrap_or_default();
    let known: HashSet<&Name> = prior.iter().collect();
    prior
        .iter()
        .chain(current.iter().filter(|tag| !known.contains(tag)))
        .cloned()
        .collect()
}

struct Pending<'s> {
    entry: TagEntry,
    pack: &'s str,
    path: &'s str,
}

pub fn build_tags(
    names: &NameTable,
    rules: TagRules,
    files: &[(Name, Vec<TagSource<'_>>)],
    prior: Option<&[Name]>,
) -> (TagTable, Vec<TagProblem>) {
    let registry = names.registry();
    let mut problems = Vec::new();

    let mut pending: BTreeMap<&Name, Vec<Pending>> = BTreeMap::new();
    for (tag, sources) in files {
        for source in sources {
            let skip = |problems: &mut Vec<TagProblem>, error: serde_json::Error| {
                log_and_push(
                    problems,
                    TagProblem::Skipped {
                        registry: registry.clone(),
                        tag: tag.clone(),
                        file: source.path.to_owned(),
                        message: format!("malformed tag file in pack {}: {error}", source.pack),
                    },
                );
            };
            // Gson reads an empty or whitespace-only file as JSON null, which creates the tag
            // before the codec refuses it; serde_json refuses it before that point.
            let blank = source
                .bytes
                .iter()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'));
            let file = match serde_json::from_slice::<TagFile>(source.bytes) {
                Ok(file) => file,
                Err(error) => {
                    // A shape error can precede a syntax error later in the file, so only a
                    // file that parses as JSON creates the tag.
                    if blank
                        || error.is_data()
                            && serde_json::from_slice::<IgnoredAny>(source.bytes).is_ok()
                    {
                        pending.entry(tag).or_default();
                    }
                    skip(&mut problems, error);
                    continue;
                }
            };
            let entries = pending.entry(tag).or_default();
            if file.replace {
                entries.clear();
            }
            entries.extend(file.values.into_iter().map(|entry| Pending {
                entry,
                pack: source.pack,
                path: source.path,
            }));
        }
    }
    let tags: Vec<(&Name, Vec<Pending>)> = pending.into_iter().collect();

    let index: HashMap<&str, usize> = tags
        .iter()
        .enumerate()
        .map(|(position, (tag, _))| (tag.as_str(), position))
        .collect();
    let mut edges: Vec<Vec<usize>> = tags
        .iter()
        .map(|(_, entries)| {
            entries
                .iter()
                .filter(|pending| pending.entry.tag && pending.entry.required)
                .filter_map(|pending| index.get(pending.entry.id.as_str()).copied())
                .collect()
        })
        .collect();
    let mut searched = FixedBitSet::with_capacity(tags.len());
    let mut frontier = Vec::new();
    for (source, (_, entries)) in tags.iter().enumerate() {
        for pending in entries
            .iter()
            .filter(|pending| pending.entry.tag && !pending.entry.required)
        {
            let Some(&target) = index.get(pending.entry.id.as_str()) else {
                continue;
            };
            if !reaches(&edges, target, source, &mut searched, &mut frontier) {
                edges[source].push(target);
            }
        }
    }

    let mut built: Vec<Option<Vec<u16>>> = vec![None; tags.len()];
    for (node, on_cycle) in dependency_order(&edges) {
        let (tag, entries) = &tags[node];
        if on_cycle {
            log_and_push(
                &mut problems,
                TagProblem::Dropped {
                    registry: registry.clone(),
                    tag: (*tag).clone(),
                    message: "it is on a cycle of nested tags".to_owned(),
                },
            );
            continue;
        }
        let mut members = Vec::new();
        let mut seen = FixedBitSet::with_capacity(names.len());
        let mut missing: Vec<&Pending> = Vec::new();
        for pending in entries {
            let entry = &pending.entry;
            if entry.tag {
                let nested = index
                    .get(entry.id.as_str())
                    .and_then(|&nested| built[nested].as_deref());
                match nested {
                    Some(nested) => {
                        for &member in nested {
                            if !seen.put(usize::from(member)) {
                                members.push(member);
                            }
                        }
                    }
                    None if entry.required => missing.push(pending),
                    None => {}
                }
            } else {
                match names.number(entry.id.as_str()) {
                    Some(member) => {
                        if !seen.put(usize::from(member)) {
                            members.push(member);
                        }
                    }
                    None if entry.required => missing.push(pending),
                    None => {}
                }
            }
        }
        if missing.is_empty() {
            built[node] = Some(members);
            continue;
        }
        let message = format!(
            "missing required references: {}",
            missing
                .iter()
                .map(|pending| format!("{} (from {})", pending.entry, pending.pack))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let first_element = missing.iter().find(|pending| !pending.entry.tag);
        match (rules, first_element) {
            (TagRules::World, Some(first)) => problems.push(TagProblem::Error {
                registry: registry.clone(),
                tag: (*tag).clone(),
                file: first.path.to_owned(),
                message,
            }),
            _ => log_and_push(
                &mut problems,
                TagProblem::Dropped {
                    registry: registry.clone(),
                    tag: (*tag).clone(),
                    message,
                },
            ),
        }
    }

    let current: BTreeSet<Name> = tags
        .iter()
        .zip(&built)
        .filter(|(_, members)| members.is_some())
        .map(|((tag, _), _)| (*tag).clone())
        .collect();
    let by_name: HashMap<&str, &[u16]> = tags
        .iter()
        .zip(&built)
        .filter_map(|((tag, _), members)| Some((tag.as_str(), members.as_deref()?)))
        .collect();

    let mut ids = number_tags(&current, prior);
    let capacity = usize::from(u16::MAX) + 1;
    if ids.len() > capacity {
        for tag in ids.split_off(capacity) {
            log_and_push(
                &mut problems,
                TagProblem::Dropped {
                    registry: registry.clone(),
                    tag,
                    message: "the registry has more tags than an id can number".to_owned(),
                },
            );
        }
    }
    let table = assemble(
        names,
        ids.into_iter().map(|tag| {
            let members = by_name
                .get(tag.as_str())
                .copied()
                .unwrap_or_default()
                .into();
            (tag, members)
        }),
    );
    (table, problems)
}

pub(crate) fn assemble(
    names: &NameTable,
    tags: impl ExactSizeIterator<Item = (Name, Box<[u16]>)>,
) -> TagTable {
    let mut table_names = Vec::with_capacity(tags.len());
    let mut numbers = HashMap::with_capacity(tags.len());
    let mut table_members = Vec::with_capacity(tags.len());
    let mut bits = Vec::with_capacity(tags.len());
    for ((tag, members), number) in tags.zip(0..=u16::MAX) {
        let mut set = FixedBitSet::with_capacity(names.len());
        set.extend(members.iter().map(|&member| usize::from(member)));
        numbers.insert(tag.clone(), number);
        table_names.push(tag);
        table_members.push(members);
        bits.push(set);
    }
    TagTable {
        registry: names.registry().clone(),
        names: table_names.into(),
        numbers,
        members: table_members.into(),
        bits: bits.into(),
    }
}

fn log_and_push(problems: &mut Vec<TagProblem>, problem: TagProblem) {
    tracing::error!("{problem}");
    problems.push(problem);
}

// chisle: one search per optional nested reference, quadratic in the tag count for a pack full of
// them; an incrementally maintained topological order lifts it.
fn reaches(
    edges: &[Vec<usize>],
    from: usize,
    to: usize,
    searched: &mut FixedBitSet,
    frontier: &mut Vec<usize>,
) -> bool {
    searched.clear();
    frontier.clear();
    searched.insert(from);
    frontier.push(from);
    while let Some(node) = frontier.pop() {
        if node == to {
            return true;
        }
        for &next in &edges[node] {
            if !searched.put(next) {
                frontier.push(next);
            }
        }
    }
    false
}

/// Explicit-stack Tarjan, so a deep chain of nested tags cannot exhaust the call stack. Nodes
/// come out dependencies first; the flag marks every node on a cycle, a self-nesting tag included.
fn dependency_order(edges: &[Vec<usize>]) -> Vec<(usize, bool)> {
    const UNVISITED: usize = usize::MAX;
    let mut order = Vec::with_capacity(edges.len());
    let mut visit = vec![UNVISITED; edges.len()];
    let mut low = vec![0; edges.len()];
    let mut on_stack = vec![false; edges.len()];
    let mut stack = Vec::new();
    let mut calls: Vec<(usize, usize)> = Vec::new();
    let mut counter = 0;
    for root in 0..edges.len() {
        if visit[root] != UNVISITED {
            continue;
        }
        visit[root] = counter;
        low[root] = counter;
        counter += 1;
        stack.push(root);
        on_stack[root] = true;
        calls.push((root, 0));
        while let Some(frame) = calls.last_mut() {
            let node = frame.0;
            if let Some(&next) = edges[node].get(frame.1) {
                frame.1 += 1;
                if visit[next] == UNVISITED {
                    visit[next] = counter;
                    low[next] = counter;
                    counter += 1;
                    stack.push(next);
                    on_stack[next] = true;
                    calls.push((next, 0));
                } else if on_stack[next] {
                    low[node] = low[node].min(visit[next]);
                }
                continue;
            }
            calls.pop();
            if let Some(&(parent, _)) = calls.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == visit[node] {
                let start = stack
                    .iter()
                    .rposition(|&member| member == node)
                    .expect("a component root is on the stack");
                let component = stack.split_off(start);
                let cyclic = component.len() > 1 || edges[node].contains(&node);
                for member in component {
                    on_stack[member] = false;
                    order.push((member, cyclic));
                }
            }
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;
    use mcrs_minecraft_core::registry_key::RegistryKey;
    use mcrs_minecraft_core::rl;

    struct TestRegistry;

    impl TestRegistry {
        const KEY: RegistryKey<TestRegistry> = RegistryKey::new(rl!("minecraft:test_registry"));
    }

    fn name(text: &str) -> Name {
        ResourceLocation::read(text).unwrap()
    }

    fn registry(entries: &[&str]) -> Registry<TestRegistry> {
        Registry::new(TestRegistry::KEY, entries.iter().map(|text| name(text))).unwrap()
    }

    type Packs<'a> = &'a [(&'a str, &'a str)];

    fn build(
        registry: &Registry<TestRegistry>,
        rules: TagRules,
        tags: &[(&str, Packs)],
        prior: Option<&[Name]>,
    ) -> (Tags<TestRegistry>, Vec<TagProblem>) {
        let paths: Vec<String> = tags
            .iter()
            .map(|(tag, _)| format!("tags/{tag}.json"))
            .collect();
        let files: Vec<(Name, Vec<TagSource>)> = tags
            .iter()
            .zip(&paths)
            .map(|((tag, packs), path)| {
                let sources = packs
                    .iter()
                    .map(|(pack, json)| TagSource {
                        pack,
                        path,
                        bytes: json.as_bytes(),
                    })
                    .collect();
                (name(tag), sources)
            })
            .collect();
        let (table, problems) = build_tags(registry.table(), rules, &files, prior);
        (Tags::new(Arc::new(table)), problems)
    }

    fn members_of(
        tags: &Tags<TestRegistry>,
        registry: &Registry<TestRegistry>,
        tag: &str,
    ) -> Option<Vec<String>> {
        let id = tags.get(&TagKey::<TestRegistry, Arc<str>>::from_location(name(tag)))?;
        Some(
            tags.members(id)
                .map(|member| registry.name(member).unwrap().to_string())
                .collect(),
        )
    }

    #[test]
    fn a_nested_tag_expands_in_place_not_in_id_order() {
        let registry = registry(&["minecraft:a", "minecraft:b", "minecraft:c", "minecraft:d"]);
        let (tags, problems) = build(
            &registry,
            TagRules::Static,
            &[
                (
                    "minecraft:outer",
                    &[(
                        "vanilla",
                        r##"{"values":["minecraft:d","#minecraft:inner","minecraft:a"]}"##,
                    )],
                ),
                (
                    "minecraft:inner",
                    &[("vanilla", r#"{"values":["minecraft:c","minecraft:b"]}"#)],
                ),
            ],
            None,
        );
        assert_eq!(problems, []);
        assert_eq!(
            members_of(&tags, &registry, "minecraft:outer"),
            Some(vec![
                "minecraft:d".to_owned(),
                "minecraft:c".to_owned(),
                "minecraft:b".to_owned(),
                "minecraft:a".to_owned()
            ])
        );

        let bit_members = |tag: &str| -> Vec<String> {
            let id = tags
                .get(&TagKey::<TestRegistry, Arc<str>>::from_location(name(tag)))
                .unwrap();
            registry
                .ids()
                .filter(|&member| tags.contains(id, member))
                .map(|member| registry.name(member).unwrap().to_string())
                .collect()
        };
        assert_eq!(
            bit_members("minecraft:outer"),
            ["minecraft:a", "minecraft:b", "minecraft:c", "minecraft:d"]
        );
        assert_eq!(
            bit_members("minecraft:inner"),
            ["minecraft:b", "minecraft:c"]
        );
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Kind {
        Dropped,
        Error,
        Skipped,
    }

    fn kinds(problems: &[TagProblem]) -> Vec<(Kind, String, String)> {
        let mut kinds: Vec<_> = problems
            .iter()
            .map(|problem| match problem {
                TagProblem::Dropped { tag, .. } => (Kind::Dropped, tag.to_string()),
                TagProblem::Error { tag, .. } => (Kind::Error, tag.to_string()),
                TagProblem::Skipped { tag, .. } => (Kind::Skipped, tag.to_string()),
            })
            .zip(problems)
            .map(|((kind, tag), problem)| (kind, tag, problem.to_string()))
            .collect();
        kinds.sort();
        kinds
    }

    struct Outcome {
        case: &'static str,
        rules: TagRules,
        tags: &'static [(&'static str, Packs<'static>)],
        members: &'static [(&'static str, Option<&'static [&'static str]>)],
        problems: &'static [(Kind, &'static str, &'static [&'static str])],
    }

    const OUTCOMES: &[Outcome] = &[
        Outcome {
            case: "static registry, missing required element: the tag is dropped, the others stay",
            rules: TagRules::Static,
            tags: &[
                (
                    "minecraft:broken",
                    &[("p", r#"{"values":["minecraft:a","minecraft:nope"]}"#)],
                ),
                ("minecraft:fine", &[("p", r#"{"values":["minecraft:b"]}"#)]),
            ],
            members: &[
                ("minecraft:broken", None),
                ("minecraft:fine", Some(&["minecraft:b"])),
            ],
            problems: &[(Kind::Dropped, "minecraft:broken", &["minecraft:nope"])],
        },
        Outcome {
            case: "world registry, missing required element: a load error naming registry, tag and file",
            rules: TagRules::World,
            tags: &[
                (
                    "minecraft:broken",
                    &[("p", r#"{"values":["minecraft:a","minecraft:nope"]}"#)],
                ),
                ("minecraft:fine", &[("p", r#"{"values":["minecraft:b"]}"#)]),
            ],
            members: &[
                ("minecraft:broken", None),
                ("minecraft:fine", Some(&["minecraft:b"])),
            ],
            problems: &[(
                Kind::Error,
                "minecraft:broken",
                &[
                    "minecraft:test_registry",
                    "#minecraft:broken",
                    "tags/minecraft:broken.json",
                    "minecraft:nope",
                ],
            )],
        },
        Outcome {
            case: "static registry, missing optional element: skipped, the tag is kept",
            rules: TagRules::Static,
            tags: &[(
                "minecraft:t",
                &[(
                    "p",
                    r#"{"values":["minecraft:a",{"id":"minecraft:nope","required":false}]}"#,
                )],
            )],
            members: &[("minecraft:t", Some(&["minecraft:a"]))],
            problems: &[],
        },
        Outcome {
            case: "world registry, missing optional element: skipped, the tag is kept",
            rules: TagRules::World,
            tags: &[(
                "minecraft:t",
                &[(
                    "p",
                    r#"{"values":["minecraft:a",{"id":"minecraft:nope","required":false}]}"#,
                )],
            )],
            members: &[("minecraft:t", Some(&["minecraft:a"]))],
            problems: &[],
        },
        Outcome {
            case: "missing required nested tag: the outer tag is dropped and logged",
            rules: TagRules::Static,
            tags: &[(
                "minecraft:outer",
                &[("p", r##"{"values":["minecraft:a","#minecraft:ghost"]}"##)],
            )],
            members: &[("minecraft:outer", None)],
            problems: &[(Kind::Dropped, "minecraft:outer", &["#minecraft:ghost"])],
        },
        Outcome {
            case: "world registry, missing required nested tag: dropped with a log, not a load error",
            rules: TagRules::World,
            tags: &[(
                "minecraft:outer",
                &[("p", r##"{"values":["minecraft:a","#minecraft:ghost"]}"##)],
            )],
            members: &[("minecraft:outer", None)],
            problems: &[(Kind::Dropped, "minecraft:outer", &["#minecraft:ghost"])],
        },
        Outcome {
            case: "missing optional nested tag: skipped",
            rules: TagRules::Static,
            tags: &[(
                "minecraft:outer",
                &[(
                    "p",
                    r##"{"values":["minecraft:a",{"id":"#minecraft:ghost","required":false}]}"##,
                )],
            )],
            members: &[("minecraft:outer", Some(&["minecraft:a"]))],
            problems: &[],
        },
        Outcome {
            case: "a drop propagates to every tag that requires the dropped tag",
            rules: TagRules::Static,
            tags: &[
                (
                    "minecraft:top",
                    &[("p", r##"{"values":["#minecraft:middle"]}"##)],
                ),
                (
                    "minecraft:middle",
                    &[("p", r#"{"values":["minecraft:nope"]}"#)],
                ),
                (
                    "minecraft:lenient",
                    &[(
                        "p",
                        r##"{"values":["minecraft:c",{"id":"#minecraft:middle","required":false}]}"##,
                    )],
                ),
            ],
            members: &[
                ("minecraft:top", None),
                ("minecraft:middle", None),
                ("minecraft:lenient", Some(&["minecraft:c"])),
            ],
            problems: &[
                (Kind::Dropped, "minecraft:middle", &["minecraft:nope"]),
                (Kind::Dropped, "minecraft:top", &["#minecraft:middle"]),
            ],
        },
        Outcome {
            case: "malformed file: skipped and logged, a well-formed file of the same tag in another pack applies; a tag whose only file is malformed does not exist",
            rules: TagRules::Static,
            tags: &[
                (
                    "minecraft:t",
                    &[
                        ("early", "{not json"),
                        ("late", r#"{"values":["minecraft:b"]}"#),
                    ],
                ),
                ("minecraft:lone", &[("p", "{not json")]),
                (
                    "minecraft:needs_lone",
                    &[("p", r##"{"values":["#minecraft:lone","minecraft:c"]}"##)],
                ),
            ],
            members: &[
                ("minecraft:t", Some(&["minecraft:b"])),
                ("minecraft:lone", None),
                ("minecraft:needs_lone", None),
            ],
            problems: &[
                (
                    Kind::Skipped,
                    "minecraft:t",
                    &["tags/minecraft:t.json", "early"],
                ),
                (
                    Kind::Skipped,
                    "minecraft:lone",
                    &["tags/minecraft:lone.json"],
                ),
                (Kind::Dropped, "minecraft:needs_lone", &["#minecraft:lone"]),
            ],
        },
        Outcome {
            case: "malformed file whose shape is wrong before its syntax breaks: the tag does not exist",
            rules: TagRules::Static,
            tags: &[("minecraft:lone", &[("p", r#"{"values":5,"#)])],
            members: &[("minecraft:lone", None)],
            problems: &[(
                Kind::Skipped,
                "minecraft:lone",
                &["tags/minecraft:lone.json"],
            )],
        },
        Outcome {
            case: "valid JSON the tag codec refuses: the tag exists with no members and a tag that requires it keeps its other members",
            rules: TagRules::Static,
            tags: &[
                ("minecraft:shape", &[("p", r#"{"values":3}"#)]),
                (
                    "minecraft:needs_shape",
                    &[("p", r##"{"values":["#minecraft:shape","minecraft:c"]}"##)],
                ),
            ],
            members: &[
                ("minecraft:shape", Some(&[])),
                ("minecraft:needs_shape", Some(&["minecraft:c"])),
            ],
            problems: &[(
                Kind::Skipped,
                "minecraft:shape",
                &["tags/minecraft:shape.json"],
            )],
        },
        Outcome {
            case: "an empty or blank file reads as JSON null: the tag exists with no members and a tag that requires it keeps its other members",
            rules: TagRules::Static,
            tags: &[
                ("minecraft:empty", &[("p", "")]),
                ("minecraft:blank", &[("p", " \r\n\t")]),
                (
                    "minecraft:needs_empty",
                    &[(
                        "p",
                        r##"{"values":["#minecraft:empty","#minecraft:blank","minecraft:c"]}"##,
                    )],
                ),
            ],
            members: &[
                ("minecraft:empty", Some(&[])),
                ("minecraft:blank", Some(&[])),
                ("minecraft:needs_empty", Some(&["minecraft:c"])),
            ],
            problems: &[
                (
                    Kind::Skipped,
                    "minecraft:empty",
                    &["tags/minecraft:empty.json"],
                ),
                (
                    Kind::Skipped,
                    "minecraft:blank",
                    &["tags/minecraft:blank.json"],
                ),
            ],
        },
        Outcome {
            case: "an optional reference back along a required one keeps both tags",
            rules: TagRules::Static,
            tags: &[
                (
                    "minecraft:outer",
                    &[("p", r##"{"values":["#minecraft:inner","minecraft:a"]}"##)],
                ),
                (
                    "minecraft:inner",
                    &[(
                        "p",
                        r##"{"values":["minecraft:b",{"id":"#minecraft:outer","required":false}]}"##,
                    )],
                ),
            ],
            members: &[
                ("minecraft:outer", Some(&["minecraft:b", "minecraft:a"])),
                ("minecraft:inner", Some(&["minecraft:b"])),
            ],
            problems: &[],
        },
        Outcome {
            case: "a tag that optionally includes itself is kept",
            rules: TagRules::Static,
            tags: &[(
                "minecraft:t",
                &[(
                    "p",
                    r##"{"values":["minecraft:a",{"id":"#minecraft:t","required":false}]}"##,
                )],
            )],
            members: &[("minecraft:t", Some(&["minecraft:a"]))],
            problems: &[],
        },
        Outcome {
            case: "two tags that optionally include each other: the first by name holds the second",
            rules: TagRules::Static,
            tags: &[
                (
                    "minecraft:first",
                    &[(
                        "p",
                        r##"{"values":[{"id":"#minecraft:second","required":false},"minecraft:a"]}"##,
                    )],
                ),
                (
                    "minecraft:second",
                    &[(
                        "p",
                        r##"{"values":[{"id":"#minecraft:first","required":false},"minecraft:b"]}"##,
                    )],
                ),
            ],
            members: &[
                ("minecraft:first", Some(&["minecraft:b", "minecraft:a"])),
                ("minecraft:second", Some(&["minecraft:b"])),
            ],
            problems: &[],
        },
    ];

    #[test]
    fn tag_outcomes_follow_vanilla() {
        let registry = registry(&["minecraft:a", "minecraft:b", "minecraft:c"]);
        for outcome in OUTCOMES {
            let (tags, problems) = build(&registry, outcome.rules, outcome.tags, None);
            for (tag, expected) in outcome.members {
                assert_eq!(
                    members_of(&tags, &registry, tag),
                    expected.map(strings),
                    "{}: members of {tag}",
                    outcome.case
                );
            }
            let found = kinds(&problems);
            assert_eq!(
                found
                    .iter()
                    .map(|(kind, tag, _)| (*kind, tag.as_str()))
                    .collect::<Vec<_>>(),
                {
                    let mut expected: Vec<_> = outcome
                        .problems
                        .iter()
                        .map(|(kind, tag, _)| (*kind, *tag))
                        .collect();
                    expected.sort();
                    expected
                },
                "{}: problems {problems:?}",
                outcome.case
            );
            for ((_, tag, text), (_, _, fragments)) in found.iter().zip({
                let mut expected = outcome.problems.to_vec();
                expected.sort();
                expected
            }) {
                for fragment in fragments {
                    assert!(
                        text.contains(fragment),
                        "{}: the report of {tag} lacks {fragment}: {text}",
                        outcome.case
                    );
                }
            }
        }
    }

    #[test]
    fn every_tag_on_a_cycle_is_dropped_and_the_build_continues() {
        let registry = registry(&["minecraft:a", "minecraft:b"]);
        let (tags, problems) = build(
            &registry,
            TagRules::Static,
            &[
                ("minecraft:a", &[("p", r##"{"values":["#minecraft:b"]}"##)]),
                ("minecraft:b", &[("p", r##"{"values":["#minecraft:a"]}"##)]),
                ("minecraft:c", &[("p", r##"{"values":["#minecraft:c"]}"##)]),
                ("minecraft:d", &[("p", r#"{"values":["minecraft:a"]}"#)]),
            ],
            None,
        );
        for tag in ["minecraft:a", "minecraft:b", "minecraft:c"] {
            assert_eq!(members_of(&tags, &registry, tag), None, "{tag} is dropped");
        }
        assert_eq!(
            members_of(&tags, &registry, "minecraft:d"),
            Some(strings(&["minecraft:a"]))
        );
        let found = kinds(&problems);
        assert_eq!(
            found
                .iter()
                .map(|(kind, tag, _)| (*kind, tag.as_str()))
                .collect::<Vec<_>>(),
            [
                (Kind::Dropped, "minecraft:a"),
                (Kind::Dropped, "minecraft:b"),
                (Kind::Dropped, "minecraft:c")
            ]
        );
        for (_, tag, text) in &found {
            assert!(text.contains("cycle"), "{tag}: {text}");
        }
    }

    #[test]
    fn a_tag_keeps_file_order_and_the_first_occurrence() {
        let registry = registry(&["minecraft:a", "minecraft:b", "minecraft:c"]);
        let (tags, problems) = build(
            &registry,
            TagRules::Static,
            &[
                (
                    "minecraft:t",
                    &[(
                        "p",
                        r##"{"values":["minecraft:c","#minecraft:n","minecraft:b","minecraft:c"]}"##,
                    )],
                ),
                (
                    "minecraft:n",
                    &[("p", r#"{"values":["minecraft:b","minecraft:a"]}"#)],
                ),
            ],
            None,
        );
        assert_eq!(problems, []);
        assert_eq!(
            members_of(&tags, &registry, "minecraft:t"),
            Some(strings(&["minecraft:c", "minecraft:b", "minecraft:a"]))
        );
    }

    #[test]
    fn an_empty_tag_has_no_members() {
        let registry = registry(&["minecraft:a", "minecraft:b"]);
        let (tags, problems) = build(
            &registry,
            TagRules::World,
            &[("minecraft:t", &[("p", r#"{"values":[]}"#)])],
            None,
        );
        assert_eq!(problems, []);
        let id = tags
            .get(&TagKey::<TestRegistry, Arc<str>>::from_location(name(
                "minecraft:t",
            )))
            .expect("the tag exists");
        assert_eq!(tags.members(id).len(), 0);
        assert!(registry.ids().all(|member| !tags.contains(id, member)));
    }

    #[test]
    fn required_false_is_honoured() {
        let registry = registry(&["minecraft:a", "minecraft:b"]);
        let (tags, problems) = build(
            &registry,
            TagRules::Static,
            &[
                (
                    "minecraft:optional",
                    &[(
                        "p",
                        r##"{"values":[{"id":"minecraft:a","required":false},{"id":"#minecraft:nested","required":false},"minecraft:b"]}"##,
                    )],
                ),
                (
                    "minecraft:nested",
                    &[("p", r#"{"values":["minecraft:b"]}"#)],
                ),
                (
                    "minecraft:explicit",
                    &[(
                        "p",
                        r#"{"values":[{"id":"minecraft:nope","required":true}]}"#,
                    )],
                ),
            ],
            None,
        );
        assert_eq!(
            members_of(&tags, &registry, "minecraft:optional"),
            Some(strings(&["minecraft:a", "minecraft:b"]))
        );
        assert_eq!(members_of(&tags, &registry, "minecraft:explicit"), None);
        assert_eq!(
            kinds(&problems)
                .into_iter()
                .map(|(kind, tag, _)| (kind, tag))
                .collect::<Vec<_>>(),
            [(Kind::Dropped, "minecraft:explicit".to_owned())]
        );
    }

    #[test]
    fn replace_true_clears_earlier_packs() {
        let registry = registry(&["minecraft:a", "minecraft:b", "minecraft:c"]);
        let (tags, problems) = build(
            &registry,
            TagRules::Static,
            &[
                (
                    "minecraft:replaced",
                    &[
                        ("p1", r#"{"values":["minecraft:a"]}"#),
                        ("p2", r#"{"values":["minecraft:b"]}"#),
                        ("p3", r#"{"replace":true,"values":["minecraft:c"]}"#),
                        ("p4", r#"{"values":["minecraft:a"]}"#),
                    ],
                ),
                (
                    "minecraft:merged",
                    &[
                        ("p1", r#"{"values":["minecraft:a"]}"#),
                        ("p2", r#"{"replace":false,"values":["minecraft:b"]}"#),
                    ],
                ),
            ],
            None,
        );
        assert_eq!(problems, []);
        assert_eq!(
            members_of(&tags, &registry, "minecraft:replaced"),
            Some(strings(&["minecraft:c", "minecraft:a"]))
        );
        assert_eq!(
            members_of(&tags, &registry, "minecraft:merged"),
            Some(strings(&["minecraft:a", "minecraft:b"]))
        );
    }

    #[test]
    fn a_tag_entry_reads_both_shapes_and_writes_them_back() {
        let cases = [
            (
                r#""minecraft:a""#,
                "minecraft:a",
                false,
                true,
                r#""minecraft:a""#,
            ),
            (
                r##""#minecraft:a""##,
                "minecraft:a",
                true,
                true,
                r##""#minecraft:a""##,
            ),
            (r#""a""#, "minecraft:a", false, true, r#""minecraft:a""#),
            (
                r#"{"id":"minecraft:a"}"#,
                "minecraft:a",
                false,
                true,
                r#""minecraft:a""#,
            ),
            (
                r##"{"id":"#mcrs:b","required":false}"##,
                "mcrs:b",
                true,
                false,
                r##"{"id":"#mcrs:b","required":false}"##,
            ),
            (
                r#"{"required":true,"id":"mcrs:b","extra":1}"#,
                "mcrs:b",
                false,
                true,
                r#""mcrs:b""#,
            ),
        ];
        for (json, id, tag, required, written) in cases {
            let entry: TagEntry = serde_json::from_str(json).unwrap();
            assert_eq!(
                entry,
                TagEntry {
                    id: name(id),
                    tag,
                    required
                },
                "{json}"
            );
            assert_eq!(serde_json::to_string(&entry).unwrap(), written, "{json}");
        }
        for bad in [
            r#"{"required":false}"#,
            r#"{"id":"minecraft:a","id":"minecraft:b"}"#,
            r#""Minecraft:A""#,
            "3",
        ] {
            assert!(serde_json::from_str::<TagEntry>(bad).is_err(), "{bad}");
        }
    }

    struct Numbering {
        case: &'static str,
        prior: Option<&'static [&'static str]>,
        tags: &'static [(&'static str, &'static str)],
        ids: &'static [&'static str],
        empty: &'static [&'static str],
    }

    const HAS_A: &str = r#"{"values":["minecraft:a"]}"#;
    const MISSING: &str = r#"{"values":["minecraft:nope"]}"#;

    const NUMBERINGS: &[Numbering] = &[
        Numbering {
            case: "no prior table: ids follow sorted names from 0",
            prior: None,
            tags: &[
                ("minecraft:c", HAS_A),
                ("minecraft:a", HAS_A),
                ("minecraft:b", HAS_A),
            ],
            ids: &["minecraft:a", "minecraft:b", "minecraft:c"],
            empty: &[],
        },
        Numbering {
            case: "a prior table keeps its ids and a new name takes the next one",
            prior: Some(&["minecraft:b", "minecraft:a"]),
            tags: &[
                ("minecraft:a", HAS_A),
                ("minecraft:b", HAS_A),
                ("minecraft:c", HAS_A),
            ],
            ids: &["minecraft:b", "minecraft:a", "minecraft:c"],
            empty: &[],
        },
        Numbering {
            case: "a removed name keeps its id with no members",
            prior: Some(&["minecraft:a", "minecraft:b"]),
            tags: &[("minecraft:a", HAS_A)],
            ids: &["minecraft:a", "minecraft:b"],
            empty: &["minecraft:b"],
        },
        Numbering {
            case: "a rename keeps the old id empty and gives the new name a new id",
            prior: Some(&["minecraft:a", "minecraft:b"]),
            tags: &[("minecraft:a", HAS_A), ("minecraft:z", HAS_A)],
            ids: &["minecraft:a", "minecraft:b", "minecraft:z"],
            empty: &["minecraft:b"],
        },
        Numbering {
            case: "new names append after every prior id, sorted among themselves",
            prior: Some(&["minecraft:m"]),
            tags: &[
                ("minecraft:y", HAS_A),
                ("minecraft:m", HAS_A),
                ("minecraft:b", HAS_A),
            ],
            ids: &["minecraft:m", "minecraft:b", "minecraft:y"],
            empty: &[],
        },
        Numbering {
            case: "a tag dropped by a failure keeps its id with no members",
            prior: Some(&["minecraft:b", "minecraft:a"]),
            tags: &[("minecraft:a", HAS_A), ("minecraft:b", MISSING)],
            ids: &["minecraft:b", "minecraft:a"],
            empty: &["minecraft:b"],
        },
    ];

    #[test]
    fn tag_ids_are_stable_with_a_prior_table() {
        let registry = registry(&["minecraft:a"]);
        for numbering in NUMBERINGS {
            let prior: Option<Vec<Name>> = numbering
                .prior
                .map(|names| names.iter().map(|text| name(text)).collect());
            let packs: Vec<[(&str, &str); 1]> = numbering
                .tags
                .iter()
                .map(|(_, json)| [("p", *json)])
                .collect();
            let tags: Vec<(&str, Packs)> = numbering
                .tags
                .iter()
                .zip(&packs)
                .map(|((tag, _), packs)| (*tag, &packs[..]))
                .collect();
            let (tags, _) = build(&registry, TagRules::Static, &tags, prior.as_deref());
            let ids: Vec<&str> = tags.table().names().iter().map(Name::as_str).collect();
            assert_eq!(ids, numbering.ids, "{}: ids in order", numbering.case);
            for tag in numbering.ids {
                let expected: &[&str] = if numbering.empty.contains(tag) {
                    &[]
                } else {
                    &["minecraft:a"]
                };
                assert_eq!(
                    members_of(&tags, &registry, tag),
                    Some(strings(expected)),
                    "{}: members of {tag}",
                    numbering.case
                );
            }
        }
    }

    fn many_tags(
        count: usize,
        json: impl Fn(usize) -> String,
    ) -> (Tags<TestRegistry>, Vec<TagProblem>) {
        let registry = registry(&["minecraft:a"]);
        let texts: Vec<(String, String)> = (0..count)
            .map(|n| (format!("minecraft:t{n:05}"), json(n)))
            .collect();
        let packs: Vec<[(&str, &str); 1]> = texts
            .iter()
            .map(|(_, json)| [("p", json.as_str())])
            .collect();
        let tags: Vec<(&str, Packs)> = texts
            .iter()
            .zip(&packs)
            .map(|((tag, _), packs)| (tag.as_str(), &packs[..]))
            .collect();
        build(&registry, TagRules::Static, &tags, None)
    }

    #[test]
    fn a_long_chain_of_nested_tags_builds_without_recursion() {
        const DEPTH: usize = 20_000;
        let (tags, problems) = many_tags(DEPTH, |n| {
            if n + 1 == DEPTH {
                HAS_A.to_owned()
            } else {
                format!(r##"{{"values":["#minecraft:t{:05}"]}}"##, n + 1)
            }
        });
        assert_eq!(problems, []);
        assert_eq!(tags.table().len(), DEPTH);
        let first = tags
            .get(&TagKey::<TestRegistry, Arc<str>>::from_location(name(
                "minecraft:t00000",
            )))
            .unwrap();
        assert_eq!(tags.members(first).len(), 1);
    }

    #[test]
    fn a_registry_with_more_tags_than_an_id_numbers_drops_the_excess() {
        let (tags, problems) =
            many_tags(usize::from(u16::MAX) + 2, |_| r#"{"values":[]}"#.to_owned());
        assert_eq!(tags.table().len(), usize::from(u16::MAX) + 1);
        assert_eq!(tags.tag_ids().len(), usize::from(u16::MAX) + 1);
        assert_eq!(tags.tag_ids().last().map(TagId::number), Some(u16::MAX));
        assert_eq!(problems.len(), 1, "{problems:?}");
        let TagProblem::Dropped { tag, message, .. } = &problems[0] else {
            panic!("expected a drop, got {:?}", problems[0]);
        };
        assert_eq!(tag.as_str(), "minecraft:t65536");
        assert!(
            message.contains("more tags than an id can number"),
            "{message}"
        );
    }
}
