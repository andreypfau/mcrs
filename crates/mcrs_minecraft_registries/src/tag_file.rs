use std::collections::{BTreeMap, HashMap, HashSet};

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{
    PackContent, PackFile, PackIndex, PackStack, StackedPack, read_json,
};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::diagnostics::{Diagnostic, Missing, Referrer};
use crate::load::Report;
use crate::order::identifier_order;
use crate::registry::Names;
use crate::tags::TagTable;

#[derive(Deserialize)]
struct TagFile {
    values: Vec<TagEntry>,
    #[serde(default)]
    replace: bool,
}

/// One value of a tag file: an entry or a `#tag`, required unless the file says otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagEntry {
    pub id: ResourceLocation,
    pub tag: bool,
    pub required: bool,
}

impl serde::Serialize for TagEntry {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let reference = match self.tag {
            true => format!("#{}", self.id),
            false => self.id.to_string(),
        };
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

fn required_by_default() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawMember {
    Plain(String),
    Full {
        id: String,
        #[serde(default = "required_by_default")]
        required: bool,
    },
}

impl<'de> Deserialize<'de> for TagEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (text, required) = match RawMember::deserialize(deserializer)? {
            RawMember::Plain(text) => (text, true),
            RawMember::Full { id, required } => (id, required),
        };
        let (tag, id) = match text.strip_prefix('#') {
            Some(id) => (true, id),
            None => (false, text.as_str()),
        };
        Ok(TagEntry {
            id: ResourceLocation::read(id).map_err(D::Error::custom)?,
            tag,
            required,
        })
    }
}

struct Node {
    name: ResourceLocation,
    members: Vec<(TagEntry, PackIndex)>,
    failed: bool,
    missing: Vec<Missing>,
}

fn missing(tag: &ResourceLocation, member: &TagEntry, pack: String) -> Missing {
    Missing {
        name: member.id.clone(),
        tag: member.tag,
        referrer: Referrer::TagFile {
            tag: tag.clone(),
            pack,
        },
    }
}

impl Node {
    fn fail(&mut self, member: &TagEntry, pack: String) {
        self.failed = true;
        self.missing.push(missing(&self.name, member, pack));
    }
}

fn targets(adjacency: &[Vec<usize>], node: usize) -> std::slice::Iter<'_, usize> {
    adjacency
        .get(node)
        .map_or([].iter(), |targets| targets.iter())
}

fn mark(seen: &mut [bool], node: usize) -> bool {
    match seen.get_mut(node) {
        Some(flag) if !*flag => {
            *flag = true;
            true
        }
        _ => false,
    }
}

fn post_order(adjacency: &[Vec<usize>]) -> Vec<usize> {
    let mut seen = vec![false; adjacency.len()];
    let mut out = Vec::with_capacity(adjacency.len());
    for start in 0..adjacency.len() {
        if !mark(&mut seen, start) {
            continue;
        }
        let mut stack = vec![(start, targets(adjacency, start))];
        while let Some((node, pending)) = stack.last_mut() {
            let node = *node;
            match pending.next() {
                Some(&target) => {
                    if mark(&mut seen, target) {
                        stack.push((target, targets(adjacency, target)));
                    }
                }
                None => {
                    out.push(node);
                    stack.pop();
                }
            }
        }
    }
    out
}

fn claim(component: &mut [Option<usize>], node: usize, root: usize) -> bool {
    match component.get_mut(node) {
        Some(slot @ None) => {
            *slot = Some(root);
            true
        }
        _ => false,
    }
}

fn reversed(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut reverse = vec![Vec::new(); adjacency.len()];
    for (from, targets) in adjacency.iter().enumerate() {
        for &to in targets {
            if let Some(sources) = reverse.get_mut(to) {
                sources.push(from);
            }
        }
    }
    reverse
}

// An edge lies on a cycle exactly when both of its ends share a component.
fn components(adjacency: &[Vec<usize>]) -> Vec<Option<usize>> {
    let reverse = reversed(adjacency);
    let mut component = vec![None; adjacency.len()];
    for &root in post_order(adjacency).iter().rev() {
        if !claim(&mut component, root, root) {
            continue;
        }
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            for &from in targets(&reverse, node) {
                if claim(&mut component, from, root) {
                    stack.push(from);
                }
            }
        }
    }
    component
}

fn same_component(component: &[Option<usize>], a: usize, b: usize) -> bool {
    matches!(
        (component.get(a), component.get(b)),
        (Some(Some(x)), Some(Some(y))) if x == y
    )
}

fn push_unique(seen: &mut [bool], out: &mut Vec<u16>, id: u16) {
    if mark(seen, usize::from(id)) {
        out.push(id);
    }
}

pub(crate) fn pack_id(stack: &PackStack, pack: PackIndex) -> String {
    stack.get(pack).map_or("", StackedPack::id).to_owned()
}

type Index = HashMap<ResourceLocation, usize>;

fn tag_target(index: &Index, member: &TagEntry) -> Option<usize> {
    index.get(&member.id).copied()
}

type RequiredEdge = (usize, TagEntry, PackIndex);

fn required_tag_edges(index: &Index, node: &Node) -> Vec<RequiredEdge> {
    node.members
        .iter()
        .filter(|(member, _)| member.tag && member.required)
        .filter_map(|(member, pack)| {
            tag_target(index, member).map(|to| (to, member.clone(), *pack))
        })
        .collect()
}

fn is_failed(nodes: &[Node], at: usize) -> bool {
    nodes.get(at).is_none_or(|node| node.failed)
}

// A value stands for an entry; a tag is read from a file only.
fn read_nodes(
    registry: &ResourceLocation,
    files: BTreeMap<ResourceLocation, Vec<PackFile<'_>>>,
    stack: &PackStack,
    skipped: &mut Vec<Diagnostic>,
) -> Vec<Node> {
    let mut ordered: Vec<(ResourceLocation, Vec<PackFile<'_>>)> = files.into_iter().collect();
    ordered.sort_by(|a, b| identifier_order(&a.0, &b.0));

    let mut nodes: Vec<Node> = Vec::new();
    for (name, files) in ordered {
        let mut exists = false;
        let mut valid: Vec<(TagFile, PackIndex)> = Vec::new();
        for file in files {
            let PackContent::Bytes(bytes) = file.content else {
                continue;
            };
            match read_json::<TagFile>(bytes) {
                Ok(tag_file) => {
                    exists = true;
                    valid.push((tag_file, file.pack));
                }
                Err(error) => {
                    exists |= !error.malformed;
                    skipped.push(Diagnostic::TagFileSkipped {
                        registry: registry.clone(),
                        tag: name.clone(),
                        pack: pack_id(stack, file.pack),
                        file: file.path.to_owned(),
                        reason: error.message,
                    });
                }
            }
        }
        if !exists {
            continue;
        }
        let start = valid
            .iter()
            .rposition(|(file, _)| file.replace)
            .unwrap_or(0);
        let members = valid
            .into_iter()
            .skip(start)
            .flat_map(|(file, pack)| file.values.into_iter().map(move |m| (m, pack)))
            .collect();
        nodes.push(Node {
            name,
            members,
            failed: false,
            missing: Vec::new(),
        });
    }
    nodes
}

fn fail_absent_members(
    nodes: &mut [Node],
    index: &Index,
    names: &Names,
    stack: &PackStack,
    absent_elements_fail_load: bool,
) -> Vec<Missing> {
    let mut unresolved = Vec::new();
    for Node {
        name,
        members,
        failed,
        missing: misses,
    } in nodes
    {
        for (member, pack) in members.iter() {
            let resolves = match member.tag {
                true => tag_target(index, member).is_some(),
                false => names.get(&member.id).is_some(),
            };
            if !member.required || resolves {
                continue;
            }
            *failed = true;
            let miss = missing(name, member, pack_id(stack, *pack));
            match member.tag || !absent_elements_fail_load {
                true => misses.push(miss),
                false => unresolved.push(miss),
            }
        }
    }
    unresolved
}

fn fail_required_cycles(
    nodes: &mut [Node],
    edges: &[Vec<RequiredEdge>],
    stack: &PackStack,
) -> Vec<Vec<usize>> {
    let required: Vec<Vec<usize>> = edges
        .iter()
        .map(|edges| edges.iter().map(|edge| edge.0).collect())
        .collect();
    let required_component = components(&required);
    for ((at, node), edges) in nodes.iter_mut().enumerate().zip(edges) {
        for (to, member, pack) in edges {
            if same_component(&required_component, *to, at) {
                node.fail(member, pack_id(stack, *pack));
            }
        }
    }
    required
}

fn propagate_failures(
    nodes: &mut [Node],
    edges: &[Vec<RequiredEdge>],
    required: &[Vec<usize>],
    stack: &PackStack,
) {
    let dependents = reversed(required);
    let mut queue: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.failed)
        .map(|(at, _)| at)
        .collect();
    while let Some(failed) = queue.pop() {
        for &holder in targets(&dependents, failed) {
            let (Some(node), Some(edges)) = (nodes.get_mut(holder), edges.get(holder)) else {
                continue;
            };
            if node.failed {
                continue;
            }
            for (to, member, pack) in edges {
                if *to == failed {
                    node.fail(member, pack_id(stack, *pack));
                }
            }
            queue.push(holder);
        }
    }
}

fn live_edges(nodes: &[Node], index: &Index, ignored: &HashSet<(usize, usize)>) -> Vec<Vec<usize>> {
    nodes
        .iter()
        .enumerate()
        .map(|(at, node)| {
            if node.failed {
                return Vec::new();
            }
            let mut targets: Vec<usize> = node
                .members
                .iter()
                .filter(|(member, _)| member.tag)
                .filter_map(|(member, _)| tag_target(index, member))
                .filter(|&to| !is_failed(nodes, to) && !ignored.contains(&(at, to)))
                .collect();
            targets.sort_unstable();
            targets.dedup();
            targets
        })
        .collect()
}

fn acyclic_live_edges(nodes: &[Node], index: &Index, required: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut ignored: HashSet<(usize, usize)> = HashSet::new();
    loop {
        let edges = live_edges(nodes, index, &ignored);
        let component = components(&edges);
        let mut chosen: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
        for (from, targets) in edges.iter().enumerate() {
            let required_here = required.get(from);
            for &to in targets {
                let optional_only = !required_here.is_some_and(|r| r.contains(&to));
                if optional_only
                    && same_component(&component, from, to)
                    && let Some(Some(root)) = component.get(from)
                {
                    let best = chosen.entry(*root).or_insert((from, to));
                    *best = (*best).max((from, to));
                }
            }
        }
        if chosen.is_empty() {
            return edges;
        }
        ignored.extend(chosen.into_values());
    }
}

fn resolve(
    nodes: &[Node],
    index: &Index,
    names: &Names,
    edges: &[Vec<usize>],
) -> Vec<Option<Vec<u16>>> {
    let mut resolved: Vec<Option<Vec<u16>>> = vec![None; nodes.len()];
    let mut seen = vec![false; names.len()];
    for at in post_order(edges) {
        let Some(node) = nodes.get(at).filter(|node| !node.failed) else {
            continue;
        };
        let live = edges.get(at);
        let mut out: Vec<u16> = Vec::new();
        for (member, _) in &node.members {
            if !member.tag {
                if let Some(id) = names.get(&member.id) {
                    push_unique(&mut seen, &mut out, id);
                }
                continue;
            }
            let Some(to) =
                tag_target(index, member).filter(|to| live.is_some_and(|l| l.contains(to)))
            else {
                continue;
            };
            let nested = resolved
                .get(to)
                .and_then(Option::as_deref)
                .unwrap_or_default();
            for &id in nested {
                push_unique(&mut seen, &mut out, id);
            }
        }
        for &id in &out {
            if let Some(flag) = seen.get_mut(usize::from(id)) {
                *flag = false;
            }
        }
        if let Some(slot) = resolved.get_mut(at) {
            *slot = Some(out);
        }
    }
    resolved
}

fn number(
    nodes: &[Node],
    index: &Index,
    mut resolved: Vec<Option<Vec<u16>>>,
    previous: Option<&TagTable>,
) -> Vec<(ResourceLocation, Option<Vec<u16>>)> {
    let mut numbered: Vec<(ResourceLocation, Option<Vec<u16>>)> = Vec::new();
    for name in previous.into_iter().flat_map(TagTable::names) {
        let members = index.get(name).and_then(|&at| resolved.get_mut(at)?.take());
        numbered.push((name.clone(), members));
    }
    for (node, slot) in nodes.iter().zip(resolved.iter_mut()) {
        if let Some(members) = slot.take() {
            numbered.push((node.name.clone(), Some(members)));
        }
    }
    numbered
}

/// The tags of `registry` its tag `files` define, numbered after the tags of `previous`.
pub(crate) fn resolve_tags(
    registry: &ResourceLocation,
    names: &Names,
    files: BTreeMap<ResourceLocation, Vec<PackFile<'_>>>,
    stack: &PackStack,
    absent_elements_fail_load: bool,
    previous: Option<&TagTable>,
    report: &mut Report,
) -> Vec<(ResourceLocation, Option<Vec<u16>>)> {
    let mut nodes = read_nodes(registry, files, stack, &mut report.skipped);
    let index: Index = nodes
        .iter()
        .enumerate()
        .map(|(at, node)| (node.name.clone(), at))
        .collect();
    let unresolved =
        fail_absent_members(&mut nodes, &index, names, stack, absent_elements_fail_load);
    if !unresolved.is_empty() {
        let missing = report.unresolved.entry(registry.clone()).or_default();
        missing.extend(unresolved);
    }
    let required_edges: Vec<_> = nodes
        .iter()
        .map(|node| required_tag_edges(&index, node))
        .collect();
    let required = fail_required_cycles(&mut nodes, &required_edges, stack);
    propagate_failures(&mut nodes, &required_edges, &required, stack);
    let edges = acyclic_live_edges(&nodes, &index, &required);
    let resolved = resolve(&nodes, &index, names, &edges);

    let failed = nodes
        .iter()
        .filter(|node| node.failed && !node.missing.is_empty());
    report
        .failed_tags
        .extend(failed.map(|node| Diagnostic::TagFailed {
            registry: registry.clone(),
            tag: node.name.clone(),
            missing: node.missing.clone(),
        }));
    number(&nodes, &index, resolved, previous)
}
