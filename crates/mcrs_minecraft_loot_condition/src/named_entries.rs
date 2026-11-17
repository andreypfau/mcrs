use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_predicate::{PredicateIssue, SmokeTable};
use mcrs_minecraft_registries::{Registries, Registry};
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition::Test;
use crate::condition_baker::ConditionBaker;
use crate::condition_file::ConditionFile;
use crate::condition_problem::ConditionProblem;
use crate::context_params::ContextParams;
use crate::entry_kind::EntryKind;
use crate::float_provider::FloatNode;
use crate::float_provider_file::FloatProviderFile;
use crate::int_provider::IntNode;
use crate::int_provider_file::IntProviderFile;
use crate::named_issue::NamedIssue;
use crate::named_source::Dependency;
use crate::provider_baker::CHAIN_LIMIT;

#[derive(Debug, Clone)]
pub struct NamedEntries {
    pub(crate) ints: Box<[Named<IntNode>]>,
    pub(crate) floats: Box<[Named<FloatNode>]>,
    pub(crate) predicates: Box<[Named<Test>]>,
    pub(crate) smoke: Result<SmokeTable, PredicateIssue>,
    pub(crate) items: ItemTable,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Named<N> {
    pub(crate) node: N,
    pub(crate) reach: Reach,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Reach {
    pub(crate) reads: ContextParams,
    pub(crate) below: usize,
}

struct Link<'r> {
    name: &'r ResourceLocationBuf,
    kind: EntryKind,
    reach: Reach,
    dependencies: Vec<Dependency>,
}

#[derive(Clone, Copy)]
struct Offsets {
    floats: usize,
    predicates: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    New,
    Open,
    Done,
}

#[derive(Clone, Copy)]
enum Outcome {
    Sound(Reach),
    Faulty { below: usize },
}

impl Outcome {
    fn reach(self) -> Reach {
        match self {
            Outcome::Sound(reach) => reach,
            Outcome::Faulty { below } => Reach {
                below,
                ..Reach::default()
            },
        }
    }
}

struct Frame {
    node: usize,
    next: usize,
}

impl NamedEntries {
    #[must_use]
    pub fn bake(
        definitions: &BlockDefinitions,
        registries: &Registries,
        items: ItemTable,
    ) -> (Self, Vec<NamedIssue>) {
        let smoke = SmokeTable::bake(registries, definitions);
        let mut issues = Vec::new();
        let mut baker = ConditionBaker::recording(definitions, registries, items, smoke.as_ref());
        let mut links = Vec::new();
        let mut ints = Vec::new();
        for (_, name, file) in registries
            .registry::<IntProviderFile>()
            .into_iter()
            .flat_map(Registry::iter)
        {
            let node = baker.int_provider(file).0;
            links.push(link(
                &mut baker,
                name,
                EntryKind::Int,
                node.reads(),
                &mut issues,
            ));
            ints.push(node);
        }
        let mut floats = Vec::new();
        for (_, name, file) in registries
            .registry::<FloatProviderFile>()
            .into_iter()
            .flat_map(Registry::iter)
        {
            let node = baker.float_provider(file).0;
            links.push(link(
                &mut baker,
                name,
                EntryKind::Float,
                node.reads(),
                &mut issues,
            ));
            floats.push(node);
        }
        let mut predicates = Vec::new();
        for (_, name, file) in registries
            .registry::<ConditionFile>()
            .into_iter()
            .flat_map(Registry::iter)
        {
            let node = baker.test(file);
            let reads = node.reads();
            links.push(link(
                &mut baker,
                name,
                EntryKind::Predicate,
                reads,
                &mut issues,
            ));
            predicates.push(node);
        }
        let offsets = Offsets {
            floats: ints.len(),
            predicates: ints.len() + floats.len(),
        };
        let reaches = check(&links, offsets, &mut issues);
        let (int_reaches, rest) = reaches.split_at(offsets.floats.min(reaches.len()));
        let (float_reaches, predicate_reaches) = rest.split_at(floats.len().min(rest.len()));
        let entries = Self {
            ints: named(ints, int_reaches, || IntNode::Fault),
            floats: named(floats, float_reaches, || FloatNode::Fault),
            predicates: named(predicates, predicate_reaches, || Test::Never),
            smoke,
            items,
        };
        (entries, issues)
    }

    pub(crate) fn reach(&self, kind: EntryKind, index: usize) -> Option<Reach> {
        match kind {
            EntryKind::Int => self.ints.get(index).map(|named| named.reach),
            EntryKind::Float => self.floats.get(index).map(|named| named.reach),
            EntryKind::Predicate => self.predicates.get(index).map(|named| named.reach),
        }
    }
}

fn link<'r>(
    baker: &mut ConditionBaker<'_>,
    name: &'r ResourceLocationBuf,
    kind: EntryKind,
    reads: ContextParams,
    issues: &mut Vec<NamedIssue>,
) -> Link<'r> {
    issues.extend(baker.drain_problems().map(|problem| NamedIssue {
        entry: name.clone(),
        kind: kind.member(),
        problem,
    }));
    Link {
        name,
        kind,
        reach: Reach {
            reads,
            below: baker.take_longest_chain(),
        },
        dependencies: baker.take_dependencies(),
    }
}

fn named<N>(nodes: Vec<N>, outcomes: &[Outcome], fault: fn() -> N) -> Box<[Named<N>]> {
    nodes
        .into_iter()
        .zip(outcomes)
        .map(|(node, outcome)| Named {
            node: match outcome {
                Outcome::Sound(_) => node,
                Outcome::Faulty { .. } => fault(),
            },
            reach: outcome.reach(),
        })
        .collect()
}

fn check(links: &[Link<'_>], offsets: Offsets, issues: &mut Vec<NamedIssue>) -> Vec<Outcome> {
    let mut marks = vec![Mark::New; links.len()];
    let mut cut = vec![false; links.len()];
    let mut outcomes = vec![Outcome::Faulty { below: 0 }; links.len()];
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
                let reach = extent(link, &outcomes, offsets);
                let outcome = match cut.get(node) {
                    Some(false) => limit(link, reach, issues),
                    _ => Outcome::Faulty { below: reach.below },
                };
                set(&mut outcomes, node, outcome);
                set(&mut marks, node, Mark::Done);
                stack.pop();
                continue;
            };
            frame.next += 1;
            let target = graph_index(dependency, offsets);
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
                    let members: Vec<usize> = stack
                        .get(start.unwrap_or(0)..)
                        .unwrap_or_default()
                        .iter()
                        .map(|frame| frame.node)
                        .collect();
                    cut_cycle(links, &members, &mut cut, issues);
                }
                Some(Mark::Done) | None => {}
            }
        }
    }
    outcomes
}

fn cut_cycle(
    links: &[Link<'_>],
    members: &[usize],
    cut: &mut [bool],
    issues: &mut Vec<NamedIssue>,
) {
    for (position, &member) in members.iter().enumerate() {
        let Some(link) = links.get(member) else {
            continue;
        };
        if cut.get(member) != Some(&false) {
            continue;
        }
        set(cut, member, true);
        let (before, after) = members.split_at(position);
        let path: Vec<&str> = after
            .iter()
            .chain(before)
            .chain([&member])
            .filter_map(|node| links.get(*node))
            .map(|link| link.name.as_str())
            .collect();
        issues.push(NamedIssue {
            entry: link.name.clone(),
            kind: link.kind.member(),
            problem: ConditionProblem::Cycle(path.join(" > ")),
        });
    }
}

fn extent(link: &Link<'_>, outcomes: &[Outcome], offsets: Offsets) -> Reach {
    let mut reach = link.reach;
    for dependency in &link.dependencies {
        let target = outcomes
            .get(graph_index(dependency, offsets))
            .map_or_else(Reach::default, |outcome| outcome.reach());
        reach.reads = reach.reads.union(target.reads);
        reach.below = reach
            .below
            .max(dependency.steps.saturating_add(target.below));
    }
    reach
}

fn limit(link: &Link<'_>, reach: Reach, issues: &mut Vec<NamedIssue>) -> Outcome {
    if reach.below >= CHAIN_LIMIT {
        issues.push(NamedIssue {
            entry: link.name.clone(),
            kind: link.kind.member(),
            problem: ConditionProblem::ChainTooLong(link.name.to_string()),
        });
        return Outcome::Faulty { below: reach.below };
    }
    Outcome::Sound(reach)
}

fn graph_index(dependency: &Dependency, offsets: Offsets) -> usize {
    match dependency.kind {
        EntryKind::Int => dependency.index,
        EntryKind::Float => offsets.floats.saturating_add(dependency.index),
        EntryKind::Predicate => offsets.predicates.saturating_add(dependency.index),
    }
}

fn set<T>(values: &mut [T], index: usize, value: T) {
    if let Some(place) = values.get_mut(index) {
        *place = value;
    }
}
