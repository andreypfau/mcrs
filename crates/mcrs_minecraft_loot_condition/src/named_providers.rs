use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_registries::{Registries, Registry};
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition_baker::ConditionBaker;
use crate::condition_problem::ConditionProblem;
use crate::context_params::ContextParams;
use crate::float_provider::FloatNode;
use crate::float_provider_file::FloatProviderFile;
use crate::int_provider::IntNode;
use crate::int_provider_file::IntProviderFile;
use crate::named_source::Dependency;
use crate::provider_baker::CHAIN_LIMIT;
use crate::provider_issue::ProviderIssue;
use crate::provider_kind::ProviderKind;

/// The entries of the registries `context_int_provider` and `context_float_provider` of one
/// load, each baked one time and addressed by the index of its registry id. A key of a provider
/// in a baked provider refers to its entry here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NamedProviders {
    pub(crate) ints: Box<[Named<IntNode>]>,
    pub(crate) floats: Box<[Named<FloatNode>]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Named<N> {
    pub(crate) node: N,
    pub(crate) reach: Reach,
}

/// What a named provider adds where a key names it: the parameters that it reads, and the most
/// members that a reference chain has below it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Reach {
    pub(crate) reads: ContextParams,
    pub(crate) below: usize,
}

/// One entry in the check across the entries, with what its own bake found.
struct Link<'r> {
    name: &'r ResourceLocationBuf,
    reach: Reach,
    dependencies: Vec<Dependency>,
}

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

impl NamedProviders {
    /// Bakes each entry of the two registries of `registries` one time and checks each entry as
    /// the start of a reference chain (LOOT-CTX-016). An entry that refers back to itself
    /// (LOOT-CTX-014), or whose chain gets more than 64 members (LOOT-CTX-017), is an issue and
    /// gives an evaluation error.
    #[must_use]
    pub fn bake(
        definitions: &BlockDefinitions,
        registries: &Registries,
    ) -> (Self, Vec<ProviderIssue>) {
        let mut issues = Vec::new();
        let mut baker = ConditionBaker::recording(definitions, registries);
        let mut links = Vec::new();
        let mut ints = Vec::new();
        let int_files = registries.registry::<IntProviderFile>();
        for (_, name, file) in int_files.into_iter().flat_map(Registry::iter) {
            let node = baker.int_provider(file).0;
            links.push(link(&mut baker, name, node.reads(), &mut issues));
            ints.push(node);
        }
        let mut floats = Vec::new();
        let float_files = registries.registry::<FloatProviderFile>();
        for (_, name, file) in float_files.into_iter().flat_map(Registry::iter) {
            let node = baker.float_provider(file).0;
            links.push(link(&mut baker, name, node.reads(), &mut issues));
            floats.push(node);
        }
        let reaches = check(&links, ints.len(), &mut issues);
        let (int_reaches, float_reaches) = reaches.split_at(ints.len().min(reaches.len()));
        let providers = Self {
            ints: named(ints, int_reaches, || IntNode::Fault),
            floats: named(floats, float_reaches, || FloatNode::Fault),
        };
        (providers, issues)
    }

    pub(crate) fn reach(&self, kind: ProviderKind, index: usize) -> Option<Reach> {
        match kind {
            ProviderKind::Int => self.ints.get(index).map(|named| named.reach),
            ProviderKind::Float => self.floats.get(index).map(|named| named.reach),
        }
    }
}

fn link<'r>(
    baker: &mut ConditionBaker<'_>,
    name: &'r ResourceLocationBuf,
    reads: ContextParams,
    issues: &mut Vec<ProviderIssue>,
) -> Link<'r> {
    issues.extend(baker.drain_problems().map(|problem| ProviderIssue {
        provider: name.clone(),
        problem,
    }));
    Link {
        name,
        reach: Reach {
            reads,
            below: baker.take_longest_chain(),
        },
        dependencies: baker.take_dependencies(),
    }
}

/// A faulty entry, `None` in `reaches`, gives an evaluation error and adds nothing below a key
/// that names it.
fn named<N>(nodes: Vec<N>, reaches: &[Option<Reach>], fault: fn() -> N) -> Box<[Named<N>]> {
    nodes
        .into_iter()
        .zip(reaches)
        .map(|(node, reach)| match reach {
            Some(reach) => Named {
                node,
                reach: *reach,
            },
            None => Named {
                node: fault(),
                reach: Reach::default(),
            },
        })
        .collect()
}

/// The reach of each entry, the int entries first; `None` for an entry that refers back to
/// itself or that starts a chain of more than 64 members. A walk in depth with an explicit stack,
/// so that a long chain of keys cannot exhaust the stack of the thread.
fn check(
    links: &[Link<'_>],
    first_float: usize,
    issues: &mut Vec<ProviderIssue>,
) -> Vec<Option<Reach>> {
    let mut marks = vec![Mark::New; links.len()];
    let mut cut = vec![false; links.len()];
    let mut reaches: Vec<Option<Reach>> = vec![None; links.len()];
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
            let Some(link) = links.get(node) else {
                stack.pop();
                continue;
            };
            let Some(dependency) = link.dependencies.get(frame.next) else {
                let reach = match cut.get(node) {
                    Some(false) => finish(link, &reaches, first_float, issues),
                    _ => None,
                };
                set(&mut reaches, node, reach);
                set(&mut marks, node, Mark::Done);
                stack.pop();
                continue;
            };
            frame.next += 1;
            let target = graph_index(dependency, first_float);
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
                    let open = stack.get(start.unwrap_or(0)..).unwrap_or_default();
                    let path: Vec<&str> = open
                        .iter()
                        .map(|frame| frame.node)
                        .chain([target])
                        .filter_map(|node| links.get(node))
                        .map(|link| link.name.as_str())
                        .collect();
                    issues.push(ProviderIssue {
                        provider: link.name.clone(),
                        problem: ConditionProblem::ProviderCycle(path.join(" > ")),
                    });
                    set(&mut cut, node, true);
                }
                Some(Mark::Done) | None => {}
            }
        }
    }
    reaches
}

/// The reach of `link` from the reaches of the entries that it names, each done; `None`, with an
/// issue, when the entry starts a chain of more than 64 members.
fn finish(
    link: &Link<'_>,
    reaches: &[Option<Reach>],
    first_float: usize,
    issues: &mut Vec<ProviderIssue>,
) -> Option<Reach> {
    let mut reach = link.reach;
    for dependency in &link.dependencies {
        let target = reaches
            .get(graph_index(dependency, first_float))
            .copied()
            .flatten()
            .unwrap_or_default();
        reach.reads = reach.reads.union(target.reads);
        reach.below = reach
            .below
            .max(dependency.steps.saturating_add(target.below));
    }
    if reach.below >= CHAIN_LIMIT {
        issues.push(ProviderIssue {
            provider: link.name.clone(),
            problem: ConditionProblem::ProviderChainTooLong(link.name.to_string()),
        });
        return None;
    }
    Some(reach)
}

fn graph_index(dependency: &Dependency, first_float: usize) -> usize {
    match dependency.kind {
        ProviderKind::Int => dependency.index,
        ProviderKind::Float => first_float.saturating_add(dependency.index),
    }
}

fn set<T>(values: &mut [T], index: usize, value: T) {
    if let Some(place) = values.get_mut(index) {
        *place = value;
    }
}
