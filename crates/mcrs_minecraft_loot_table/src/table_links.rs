use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_minecraft_loot_condition::ContextParams;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::issue_owner::IssueOwner;
use crate::loot_issue::{LootIssue, LootProblem};
use crate::table::Table;

#[derive(Debug, Clone)]
pub(crate) struct TableLinks {
    pub(crate) owner: usize,
    pub(crate) kind: IssueOwner,
    pub(crate) checked_against: ContextKeySet,
    pub(crate) inline: bool,
    pub(crate) reads: ContextParams,
    pub(crate) edges: Vec<Edge>,
    pub(crate) chains: Vec<ChainPlace>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChainPlace {
    pub(crate) path: String,
    pub(crate) length: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Edge {
    pub(crate) slot: usize,
    pub(crate) path: String,
}

pub(crate) const MAX_NESTING: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    New,
    Open,
    Done,
}

struct Frame {
    node: usize,
    next: usize,
}

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

struct DeepLink {
    node: usize,
    slot: usize,
}

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
