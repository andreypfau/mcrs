use mcrs_minecraft_enchantment::level_based_value::LevelBasedValue;
use mcrs_minecraft_nbt_path::NbtPath;
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::condition::Condition;
use crate::context_param::ContextParam;
use crate::context_params::ContextParams;
use crate::dispatch_case::DispatchCase;
use crate::environment_read::EnvironmentRead;
use crate::int_provider::{IntNode, environment_reads, union};
use crate::loot_inputs::LootInputs;
use crate::loot_world::LootWorld;
use crate::provider_evaluation::Evaluation;
use crate::weighted_entry::WeightedEntry;

#[derive(Debug, Clone, PartialEq)]
pub struct FloatProvider(pub(crate) FloatNode);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FloatNode {
    Fault,
    Constant(f32),
    Named {
        index: usize,
        reads: ContextParams,
    },
    Unary(FloatUnary, Box<FloatNode>),
    FromInt(Box<IntNode>),
    Binary(FloatBinary, Box<FloatNode>, Box<FloatNode>),
    Mod(Box<FloatNode>, Box<FloatNode>),
    Pow(Box<FloatNode>, Box<FloatNode>),
    Uniform(Box<FloatNode>, Box<FloatNode>),
    Aggregate(FloatAggregate, Box<[FloatNode]>),
    Conditional {
        condition: Box<Condition>,
        on_true: Box<FloatNode>,
        on_false: Box<FloatNode>,
    },
    Dispatcher {
        cases: Box<[DispatchCase<FloatNode>]>,
        default: Box<FloatNode>,
    },
    WeightedList {
        total: i32,
        entries: Box<[WeightedEntry<FloatNode>]>,
    },
    Environment(EnvironmentRead<EnvironmentNumber>),
    Storage {
        storage: ResourceLocationBuf,
        path: NbtPath,
        fallback: Box<FloatNode>,
    },
    EnchantmentLevel(LevelBasedValue),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum EnvironmentNumber {
    Int(i32),
    Float(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatUnary {
    Abs,
    Negate,
    Floor,
    Ceil,
    Round,
    Truncate,
    Sqrt,
    Sin,
    Cos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatBinary {
    Sub,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatAggregate {
    Add,
    Mul,
    Min,
    Max,
    Avg,
    Length,
}

impl FloatProvider {
    #[must_use]
    pub const fn constant(value: f32) -> Self {
        Self(FloatNode::Constant(value))
    }

    pub fn read<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> f32 {
        Evaluation::new(inputs, random, scratch)
            .float(&self.0)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
    }

    #[must_use]
    pub fn reads(&self) -> ContextParams {
        self.0.reads()
    }
}

impl FloatNode {
    pub(crate) fn reads(&self) -> ContextParams {
        match self {
            FloatNode::Fault | FloatNode::Constant(_) => ContextParams::EMPTY,
            FloatNode::Named { reads, .. } => *reads,
            FloatNode::Unary(_, input) => input.reads(),
            FloatNode::FromInt(input) => input.reads(),
            FloatNode::Binary(_, first, second)
            | FloatNode::Mod(first, second)
            | FloatNode::Pow(first, second)
            | FloatNode::Uniform(first, second) => first.reads().union(second.reads()),
            FloatNode::Aggregate(_, inputs) => union(inputs.iter().map(FloatNode::reads)),
            FloatNode::Conditional {
                condition,
                on_true,
                on_false,
            } => condition
                .reads()
                .union(on_true.reads())
                .union(on_false.reads()),
            FloatNode::Dispatcher { cases, default } => union(
                cases
                    .iter()
                    .map(|case| case.condition.reads().union(case.value.reads())),
            )
            .union(default.reads()),
            FloatNode::WeightedList { entries, .. } => {
                union(entries.iter().map(|entry| entry.data.reads()))
            }
            FloatNode::Environment(read) => environment_reads(read.positional),
            FloatNode::Storage { fallback, .. } => fallback.reads(),
            FloatNode::EnchantmentLevel(_) => ContextParams::of(&[ContextParam::EnchantmentLevel]),
        }
    }
}
