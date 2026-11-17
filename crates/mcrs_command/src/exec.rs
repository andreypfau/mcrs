use crate::graph::Graph;
use crate::node_id::NodeId;
use crate::parse::{Entry, ParseResult, contexts};
use crate::span::Span;
use alloc::vec::Vec;
use core::fmt;
use core::mem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage<H> {
    pub modifier: Option<H>,
    pub forks: bool,
    pub command: Option<H>,
}

#[derive(Clone, Debug)]
pub struct Chain<H> {
    entries: Vec<Entry>,
    stages: Vec<Stage<H>>,
}

impl<H> Default for Chain<H> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            stages: Vec::new(),
        }
    }
}

impl<H: Copy> Chain<H> {
    pub fn copy_from<A, R, E: Copy>(
        &mut self,
        graph: &Graph<A, R, H>,
        parse: &ParseResult<'_, '_, E>,
    ) {
        self.entries.clear();
        self.entries.extend_from_slice(parse.entries());
        self.stages.clear();
        let stages = contexts(&self.entries).map(|context| {
            let node = context.last().and_then(|(node, _)| graph.node(node));
            Stage {
                modifier: node.and_then(|node| node.modifier().copied()),
                forks: node.is_some_and(|node| node.forks()),
                command: node.and_then(|node| node.command().copied()),
            }
        });
        self.stages.extend(stages);
    }
}

impl<H> Chain<H> {
    pub fn stages(&self) -> &[Stage<H>] {
        &self.stages
    }

    pub fn span(&self, stage: usize, node: NodeId) -> Option<Span> {
        contexts(&self.entries)
            .nth(stage)?
            .nodes()
            .find(|&(parsed, _)| parsed == node)
            .map(|(_, span)| span)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub forks: usize,
    pub sequence: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            forks: 65536,
            sequence: 65536,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure<E> {
    Command(E),
    ForkLimit(usize),
}

impl<E: fmt::Display> fmt::Display for Failure<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => error.fmt(f),
            Self::ForkLimit(limit) => write!(f, "a source list reached {limit} sources"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> core::error::Error for Failure<E> {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum End {
    Completed,
    Failed,
    ForkLimit,
    SequenceLimit,
}

pub trait Executor<S, H> {
    type Error;

    fn modify(
        &mut self,
        stage: usize,
        modifier: &H,
        source: &S,
        out: &mut Vec<S>,
    ) -> Result<(), Self::Error>;

    fn run(&mut self, stage: usize, command: &H, source: &S) -> Result<i32, Self::Error>;

    fn report(&mut self, failure: Failure<Self::Error>);
}

#[derive(Debug)]
pub struct SourceLists<S> {
    current: Vec<S>,
    next: Vec<S>,
}

impl<S> Default for SourceLists<S> {
    fn default() -> Self {
        Self {
            current: Vec::new(),
            next: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resume<S> {
    pub stage: usize,
    pub sources: Vec<S>,
    pub cost: usize,
}

pub fn execute<S, H, X: Executor<S, H>>(
    stages: &[Stage<H>],
    source: S,
    limits: Limits,
    executor: &mut X,
    lists: &mut SourceLists<S>,
) -> End {
    lists.current.clear();
    lists.current.push(source);
    run_from(stages, 0, 0, limits, executor, lists)
}

pub fn resume<S, H, X: Executor<S, H>>(
    stages: &[Stage<H>],
    resume: Resume<S>,
    limits: Limits,
    executor: &mut X,
    lists: &mut SourceLists<S>,
) -> End {
    lists.current.clear();
    lists.current.extend(resume.sources);
    run_from(stages, resume.stage, resume.cost, limits, executor, lists)
}

fn run_from<S, H, X: Executor<S, H>>(
    stages: &[Stage<H>],
    start: usize,
    mut cost: usize,
    limits: Limits,
    executor: &mut X,
    lists: &mut SourceLists<S>,
) -> End {
    let SourceLists { current, next } = lists;
    let Some((last, modifiers)) = stages.split_last() else {
        return End::Failed;
    };
    if start > modifiers.len() {
        return End::Failed;
    }
    let mut forked = modifiers.iter().take(start).any(|stage| stage.forks);
    for (index, stage) in modifiers.iter().enumerate().skip(start) {
        forked |= stage.forks;
        let Some(modifier) = &stage.modifier else {
            continue;
        };
        cost += 1;
        next.clear();
        for source in current.iter() {
            let before = next.len();
            if let Err(error) = executor.modify(index, modifier, source, next) {
                next.truncate(before);
                if !forked {
                    executor.report(Failure::Command(error));
                    return End::Failed;
                }
            }
            if next.len() >= limits.forks {
                if !forked {
                    executor.report(Failure::ForkLimit(limits.forks));
                }
                return End::ForkLimit;
            }
        }
        mem::swap(current, next);
        if current.is_empty() {
            return End::Completed;
        }
    }
    let Some(command) = &last.command else {
        return End::Failed;
    };
    let budget = limits.sequence.max(1);
    for source in current.iter() {
        if cost >= budget {
            return End::SequenceLimit;
        }
        cost += 1;
        if let Err(error) = executor.run(modifiers.len(), command, source)
            && !forked
        {
            executor.report(Failure::Command(error));
        }
    }
    End::Completed
}
