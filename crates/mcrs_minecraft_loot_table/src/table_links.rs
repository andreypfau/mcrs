use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::ContextParams;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::issue_owner::IssueOwner;
use crate::loot_issue::{LootIssue, LootProblem};
use crate::table::Table;

/// What the bake knows of one baked table for the checks across tables. An entry of the registry
/// `item_modifier` is a table with no pool, whose modifiers are the entry (LOOT-CTX-017).
#[derive(Debug, Clone)]
pub(crate) struct TableLinks {
    /// The index of the table of the registry that holds the table: itself, or the table that
    /// holds an inline table.
    pub(crate) owner: usize,
    pub(crate) kind: IssueOwner,
    /// The type that the parts of the table were checked against: its own type, or the type of
    /// the table that holds an inline table.
    pub(crate) checked_against: ContextKeySet,
    pub(crate) inline: bool,
    /// The parameters that the parts of the table read, without its nested tables.
    pub(crate) reads: ContextParams,
    pub(crate) edges: Vec<Edge>,
    pub(crate) chains: Vec<ChainPlace>,
}

/// A place of a provider in a table, and the most members that a reference chain gets below the
/// place: the inline parts and the named providers of the provider.
#[derive(Debug, Clone)]
pub(crate) struct ChainPlace {
    pub(crate) path: String,
    pub(crate) length: usize,
}

/// One element of the list `nested` of a table, and the path of its `loot_table` entry.
#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub(crate) slot: usize,
    pub(crate) path: String,
}

/// The most tables in one chain of nested tables. A roll recurses once for each table of the
/// chain, so the bake cuts a longer chain as it cuts a cycle.
pub(crate) const MAX_NESTING: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    New,
    Open,
    Done,
}

/// A table of the walk and the index of its next edge.
struct Frame {
    node: usize,
    next: usize,
}

/// Reports and cuts each reference back to a table that contains it (LOOT-CTX-014), checks each
/// nested table of the registry against the type of the table that contains it (LOOT-CTX-013),
/// and reports each place where a reference chain gets more than [`MAX_NESTING`] members
/// (LOOT-CTX-017). Then cuts each chain of nested tables that is still too long, so that each
/// evaluation ends in a bounded depth.
pub(crate) fn check(
    links: &[TableLinks],
    tables: &mut [Table],
    names: &[ResourceLocationBuf],
    issues: &mut Vec<LootIssue>,
) {
    let mut marks = vec![Mark::New; links.len()];
    let mut reads = vec![ContextParams::EMPTY; links.len()];
    let mut depths = vec![0; links.len()];
    let mut finished = Vec::with_capacity(links.len());
    let mut deep = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    for root in 0..links.len() {
        if marks.get(root) != Some(&Mark::New) {
            continue;
        }
        set(&mut marks, root, Mark::Open);
        stack.push(Frame {
            node: root,
            next: 0,
        });
        while let Some(frame) = stack.last_mut() {
            let node = frame.node;
            let Some(edge) = links.get(node).and_then(|link| link.edges.get(frame.next)) else {
                let depth = deep_links(links, tables, &depths, node, &mut deep);
                set(&mut depths, node, depth);
                let closed = target_reads(links, tables, &reads, node);
                set(&mut reads, node, closed);
                set(&mut marks, node, Mark::Done);
                finished.push(node);
                stack.pop();
                continue;
            };
            frame.next += 1;
            let Some(target) = nested(tables, node, edge.slot) else {
                continue;
            };
            match marks.get(target) {
                Some(Mark::New) => {
                    set(&mut marks, target, Mark::Open);
                    stack.push(Frame {
                        node: target,
                        next: 0,
                    });
                }
                Some(Mark::Open) => {
                    let start = stack.iter().position(|frame| frame.node == target);
                    let mut cycle: Vec<ResourceLocationBuf> = Vec::new();
                    let open = stack.get(start.unwrap_or(0)..).unwrap_or_default();
                    for frame in open {
                        let name = links.get(frame.node).and_then(|link| names.get(link.owner));
                        if let Some(name) = name
                            && cycle.last() != Some(name)
                        {
                            cycle.push(name.clone());
                        }
                    }
                    report(
                        links,
                        names,
                        node,
                        &edge.path,
                        LootProblem::Cycle(cycle),
                        issues,
                    );
                    cut(tables, node, edge.slot);
                }
                Some(Mark::Done) | None => {}
            }
        }
    }
    for (node, link) in links.iter().enumerate() {
        for edge in &link.edges {
            let Some(target) = nested(tables, node, edge.slot) else {
                continue;
            };
            if links.get(target).is_none_or(|target| target.inline) {
                continue;
            }
            let read = reads.get(target).copied().unwrap_or_default();
            if let Err(problem) = read.check(link.checked_against) {
                let problem = LootProblem::ParamNotAllowed(problem);
                report(links, names, node, &edge.path, problem, issues);
            }
        }
    }
    check_chains(links, tables, names, &finished, issues);
    for edge in deep {
        cut(tables, edge.node, edge.slot);
    }
}

/// Reports each link to a nested table and each place of a provider where the longest chain from
/// some start gets more than [`MAX_NESTING`] members. `finished` holds each table after the tables
/// that it nests, so its reverse visits each table after each table that leads to it.
fn check_chains(
    links: &[TableLinks],
    tables: &[Table],
    names: &[ResourceLocationBuf],
    finished: &[usize],
    issues: &mut Vec<LootIssue>,
) {
    let mut above = vec![1_usize; links.len()];
    for &node in finished.iter().rev() {
        let Some(link) = links.get(node) else {
            continue;
        };
        let length = above.get(node).copied().unwrap_or(1);
        for edge in &link.edges {
            let Some(target) = nested(tables, node, edge.slot) else {
                continue;
            };
            if length >= MAX_NESTING {
                report(links, names, node, &edge.path, LootProblem::TooDeep, issues);
            }
            if let Some(place) = above.get_mut(target) {
                *place = (*place).max(length.saturating_add(1));
            }
        }
        for chain in &link.chains {
            if length.saturating_add(chain.length) > MAX_NESTING {
                let problem = LootProblem::ChainTooLong;
                report(links, names, node, &chain.path, problem, issues);
            }
        }
    }
}

/// A link to a nested table that the walk cuts after the checks.
struct DeepLink {
    node: usize,
    slot: usize,
}

/// Adds to `deep` each link of the table at `node` to a table at the depth [`MAX_NESTING`]; gives
/// the depth of `node`, which each of its other nested tables has given.
fn deep_links(
    links: &[TableLinks],
    tables: &[Table],
    depths: &[usize],
    node: usize,
    deep: &mut Vec<DeepLink>,
) -> usize {
    let Some(link) = links.get(node) else {
        return 1;
    };
    let mut deepest = 0;
    for edge in &link.edges {
        let Some(target) = nested(tables, node, edge.slot) else {
            continue;
        };
        let depth = depths.get(target).copied().unwrap_or(0);
        if depth >= MAX_NESTING {
            deep.push(DeepLink {
                node,
                slot: edge.slot,
            });
        } else {
            deepest = deepest.max(depth);
        }
    }
    deepest.saturating_add(1)
}

fn cut(tables: &mut [Table], node: usize, slot: usize) {
    if let Some(place) = tables
        .get_mut(node)
        .and_then(|table| table.nested.get_mut(slot))
    {
        *place = None;
    }
}

/// The parameters that the table at `node` and its nested tables read; each nested table that
/// the cut leaves is done.
fn target_reads(
    links: &[TableLinks],
    tables: &[Table],
    reads: &[ContextParams],
    node: usize,
) -> ContextParams {
    let Some(link) = links.get(node) else {
        return ContextParams::EMPTY;
    };
    link.edges
        .iter()
        .filter_map(|edge| nested(tables, node, edge.slot))
        .filter_map(|target| reads.get(target))
        .fold(link.reads, |all, &read| all.union(read))
}

fn nested(tables: &[Table], node: usize, slot: usize) -> Option<usize> {
    let target = tables.get(node)?.nested.get(slot).copied().flatten()?;
    usize::try_from(target).ok()
}

fn set<T>(values: &mut [T], index: usize, value: T) {
    if let Some(place) = values.get_mut(index) {
        *place = value;
    }
}

fn report(
    links: &[TableLinks],
    names: &[ResourceLocationBuf],
    node: usize,
    path: &str,
    problem: LootProblem,
    issues: &mut Vec<LootIssue>,
) {
    let Some(link) = links.get(node) else {
        return;
    };
    let Some(table) = names.get(link.owner) else {
        return;
    };
    issues.push(LootIssue {
        name: table.clone(),
        owner: link.kind,
        path: path.to_owned(),
        problem,
    });
}
