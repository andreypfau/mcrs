use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition::Condition;
use crate::context_param::ContextParam;
use crate::context_params::ContextParams;
use crate::dispatch_case::DispatchCase;
use crate::environment_read::EnvironmentRead;
use crate::float_provider::FloatNode;
use crate::loot_inputs::LootInputs;
use crate::loot_world::LootWorld;
use crate::provider_evaluation::Evaluation;
use crate::score_target::ScoreTarget;
use crate::weighted_entry::WeightedEntry;

#[derive(Debug, Clone, PartialEq)]
pub struct IntProvider(pub(crate) IntNode);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum IntNode {
    Fault,
    Constant(i32),
    Named {
        index: usize,
        reads: ContextParams,
    },
    Unary(IntUnary, Box<IntNode>),
    FromFloat(Box<FloatNode>),
    Binary(IntBinary, Box<IntNode>, Box<IntNode>),
    Pow(Box<IntNode>, Box<IntNode>),
    Uniform(Box<IntNode>, Box<IntNode>),
    Aggregate(IntAggregate, Box<[IntNode]>),
    Conditional {
        condition: Box<Condition>,
        on_true: Box<IntNode>,
        on_false: Box<IntNode>,
    },
    Dispatcher {
        cases: Box<[DispatchCase<IntNode>]>,
        default: Box<IntNode>,
    },
    WeightedList {
        total: i32,
        entries: Box<[WeightedEntry<IntNode>]>,
    },
    Environment(EnvironmentRead<i32>),
    Storage {
        storage: ResourceLocationBuf,
        path: NbtPath,
        fallback: Box<IntNode>,
    },
    Binomial(Box<IntNode>, Box<FloatNode>),
    Score {
        target: ScoreTarget,
        objective: Box<str>,
        fallback: Box<IntNode>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntUnary {
    Abs,
    Negate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntBinary {
    Sub,
    Mod,
    FloorMod,
    Div,
    FloorDiv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntAggregate {
    Add,
    Mul,
    Min,
    Max,
    Avg,
}

impl IntProvider {
    #[must_use]
    pub const fn constant(value: i32) -> Self {
        Self(IntNode::Constant(value))
    }

    pub fn read<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> i32 {
        Evaluation::new(inputs, random, scratch)
            .int(&self.0)
            .unwrap_or(0)
    }

    #[must_use]
    pub fn reads(&self) -> ContextParams {
        self.0.reads()
    }
}

impl IntNode {
    pub(crate) fn reads(&self) -> ContextParams {
        match self {
            IntNode::Fault | IntNode::Constant(_) => ContextParams::EMPTY,
            IntNode::Named { reads, .. } => *reads,
            IntNode::Unary(_, input) => input.reads(),
            IntNode::FromFloat(input) => input.reads(),
            IntNode::Binary(_, first, second)
            | IntNode::Pow(first, second)
            | IntNode::Uniform(first, second) => first.reads().union(second.reads()),
            IntNode::Aggregate(_, inputs) => union(inputs.iter().map(IntNode::reads)),
            IntNode::Conditional {
                condition,
                on_true,
                on_false,
            } => condition
                .reads()
                .union(on_true.reads())
                .union(on_false.reads()),
            IntNode::Dispatcher { cases, default } => union(
                cases
                    .iter()
                    .map(|case| case.condition.reads().union(case.value.reads())),
            )
            .union(default.reads()),
            IntNode::WeightedList { entries, .. } => {
                union(entries.iter().map(|entry| entry.data.reads()))
            }
            IntNode::Environment(read) => environment_reads(read.positional),
            IntNode::Storage { fallback, .. } => fallback.reads(),
            IntNode::Binomial(n, p) => n.reads().union(p.reads()),
            IntNode::Score {
                target, fallback, ..
            } => score_reads(target).union(fallback.reads()),
        }
    }
}

pub(crate) fn union(reads: impl Iterator<Item = ContextParams>) -> ContextParams {
    reads.fold(ContextParams::EMPTY, ContextParams::union)
}

pub(crate) fn environment_reads(positional: bool) -> ContextParams {
    match positional {
        true => ContextParams::of(&[ContextParam::Origin]),
        false => ContextParams::EMPTY,
    }
}

fn score_reads(target: &ScoreTarget) -> ContextParams {
    match target {
        ScoreTarget::Entity(param) => ContextParams::of(&[*param]),
        ScoreTarget::Name(_) => ContextParams::EMPTY,
    }
}
