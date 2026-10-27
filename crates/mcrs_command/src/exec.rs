use crate::error::Span;
use crate::parse::{ContextView, Entry, ParseResult, contexts};
use crate::tree::{Graph, NodeId};
use alloc::vec::Vec;

/// What one stage of a chain runs: the modifier of the redirect it follows, or the command of
/// its last node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage<H> {
    pub modifier: Option<H>,
    pub forks: bool,
    pub command: Option<H>,
}

/// The contexts of one parse and their stages, owned so that the run can outlive the parse.
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
        self.fill_stages(graph);
    }

    fn fill_stages<A, R>(&mut self, graph: &Graph<A, R, H>) {
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
    /// Each modifier stage, then the run stage.
    pub fn stages(&self) -> &[Stage<H>] {
        &self.stages
    }

    pub fn context(&self, stage: usize) -> Option<ContextView<'_>> {
        contexts(&self.entries).nth(stage)
    }

    /// The span of `node` in the stage `stage`.
    pub fn span(&self, stage: usize, node: NodeId) -> Option<Span> {
        self.context(stage)?
            .nodes()
            .find(|&(parsed, _)| parsed == node)
            .map(|(_, span)| span)
    }
}

/// The limits of one run of a chain (CMD-EX-036, CMD-EX-037).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// A source list never holds this many sources.
    pub forks: usize,
    /// The cost of the stages and the runs; a value below 1 counts as 1.
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

/// What the output of the original source receives when a stage that is not forked fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure<E> {
    Command(E),
    ForkLimit(usize),
}

/// How a run of a chain ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Every source of the last list ran, or a list became empty.
    Completed,
    /// A stage that is not forked failed, or the chain has no command to run.
    Failed,
    ForkLimit,
    /// The run stopped before a run of the run stage; the output receives no message.
    SequenceLimit,
}

/// What the host does for each stage of a chain. `stage` is the index of the context.
pub trait Executor<S, H> {
    type Error;

    /// Adds the sources that the modifier gives for `source` to `out`.
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

/// The source lists of a run, owned by the caller and reused between runs.
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

/// Runs each modifier stage over the whole source list, then the run stage for each source of
/// the last list (CMD-EX-030 to CMD-EX-037).
pub fn execute<S, H, X: Executor<S, H>>(
    stages: &[Stage<H>],
    source: S,
    limits: Limits,
    executor: &mut X,
    lists: &mut SourceLists<S>,
) -> End {
    let SourceLists { current, next } = lists;
    current.clear();
    current.push(source);
    let Some((last, modifiers)) = stages.split_last() else {
        return End::Failed;
    };
    let mut forked = false;
    let mut cost = 0usize;
    for (index, stage) in modifiers.iter().enumerate() {
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
        core::mem::swap(current, next);
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
