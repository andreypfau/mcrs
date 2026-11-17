use glam::DVec3;
use mcrs_minecraft_environment::AttributeValue;
use mcrs_minecraft_numeric::{
    ceil_to_i32, cos_nearest, floor_div, floor_mod, max_f32, min_f32, pow_f32, sin_nearest,
};
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;

use crate::condition::Condition;
use crate::dispatch_case::DispatchCase;
use crate::draw;
use crate::environment_read::EnvironmentRead;
use crate::float_provider::{
    EnvironmentNumber, FloatAggregate, FloatBinary, FloatNode, FloatUnary,
};
use crate::int_provider::{IntAggregate, IntBinary, IntNode, IntUnary};
use crate::loot_inputs::LootInputs;
use crate::loot_world::LootWorld;
use crate::score_target::ScoreTarget;
use crate::score_view::ScoreHolder;
use crate::weighted_entry::WeightedEntry;

pub(crate) struct Evaluation<'e, 'i, 'c, E, W, R> {
    inputs: LootInputs<'i, 'c, E, W>,
    random: &'e mut R,
    scratch: &'e mut PredicateScratch,
}

impl<'e, 'i, 'c, E: Copy, W: LootWorld<E>, R: Random> Evaluation<'e, 'i, 'c, E, W, R> {
    pub(crate) fn new(
        inputs: LootInputs<'i, 'c, E, W>,
        random: &'e mut R,
        scratch: &'e mut PredicateScratch,
    ) -> Self {
        Self {
            inputs,
            random,
            scratch,
        }
    }

    pub(crate) fn int(&mut self, node: &IntNode) -> Option<i32> {
        match node {
            IntNode::Fault => None,
            IntNode::Constant(value) => Some(*value),
            IntNode::Named { index, .. } => {
                let named = self.inputs.named;
                self.int(&named.ints.get(*index)?.node)
            }
            IntNode::Unary(IntUnary::Abs, input) => self.int(input)?.checked_abs(),
            IntNode::Unary(IntUnary::Negate, input) => self.int(input)?.checked_neg(),
            IntNode::FromFloat(input) => from_float(self.float(input)?),
            IntNode::Binary(op, left, right) => {
                let a = self.int(left)?;
                let b = self.int(right)?;
                int_binary(*op, a, b)
            }
            IntNode::Pow(base, exponent) => {
                let a = self.int(base)?;
                let b = self.int(exponent)?;
                int_pow(a, b)
            }
            IntNode::Uniform(min, max) => {
                let a = self.int(min)?;
                let b = self.int(max)?;
                draw::between(self.random, a, b)
            }
            IntNode::Aggregate(op, inputs) => self.int_aggregate(*op, inputs),
            IntNode::Conditional {
                condition,
                on_true,
                on_false,
            } => {
                let selected = self.conditional(condition, on_true, on_false);
                self.int(selected)
            }
            IntNode::Dispatcher { cases, default } => {
                let selected = self.dispatch(cases, default);
                self.int(selected)
            }
            IntNode::WeightedList { total, entries } => {
                let selected = self.weighted(*total, entries)?;
                self.int(selected)
            }
            IntNode::Environment(read) => Some(self.environment_int(read)),
            IntNode::Storage {
                storage,
                path,
                fallback,
            } => match self
                .inputs
                .world
                .storage_number(storage.as_location(), path)
            {
                Some(number) => Some(number.to_i32()),
                None => self.int(fallback),
            },
            IntNode::Binomial(n, p) => {
                let trials = self.int(n)?;
                let chance = self.float(p)?;
                chance
                    .is_finite()
                    .then(|| draw::binomial(self.random, trials, chance))
            }
            IntNode::Score {
                target,
                objective,
                fallback,
            } => match self.score(target, objective) {
                Some(score) => Some(score),
                None => self.int(fallback),
            },
        }
    }

    pub(crate) fn float(&mut self, node: &FloatNode) -> Option<f32> {
        match node {
            FloatNode::Fault => None,
            FloatNode::Constant(value) => Some(*value),
            FloatNode::Named { index, .. } => {
                let named = self.inputs.named;
                self.float(&named.floats.get(*index)?.node)
            }
            FloatNode::Unary(op, input) => self.float(input).map(|a| float_unary(*op, a)),
            #[expect(clippy::cast_precision_loss, reason = "toF32 of an i32")]
            FloatNode::FromInt(input) => self.int(input).map(|a| a as f32),
            FloatNode::Mod(left, right) => {
                let b = self.float(right)?;
                if b == 0.0 {
                    return Some(f32::NAN);
                }
                Some(self.float(left)? % b)
            }
            FloatNode::Binary(op, left, right) => {
                let a = self.float(left)?;
                let b = self.float(right)?;
                Some(match op {
                    FloatBinary::Sub => a - b,
                    FloatBinary::Div => a / b,
                })
            }
            FloatNode::Pow(base, exponent) => {
                let a = self.float(base)?;
                let b = self.float(exponent)?;
                Some(match a == 0.0 && b == 0.0 {
                    true => f32::NAN,
                    false => pow_f32(a, b),
                })
            }
            FloatNode::Uniform(min, max) => {
                let a = self.float(min)?;
                let b = self.float(max)?;
                Some(match a >= b {
                    true => a,
                    false => self.random.next_f32() * (b - a) + a,
                })
            }
            FloatNode::Aggregate(op, inputs) => self.float_aggregate(*op, inputs),
            FloatNode::Conditional {
                condition,
                on_true,
                on_false,
            } => {
                let selected = self.conditional(condition, on_true, on_false);
                self.float(selected)
            }
            FloatNode::Dispatcher { cases, default } => {
                let selected = self.dispatch(cases, default);
                self.float(selected)
            }
            FloatNode::WeightedList { total, entries } => {
                let selected = self.weighted(*total, entries)?;
                self.float(selected)
            }
            FloatNode::Environment(read) => Some(self.environment_float(read)),
            FloatNode::Storage {
                storage,
                path,
                fallback,
            } => match self
                .inputs
                .world
                .storage_number(storage.as_location(), path)
            {
                Some(number) => Some(number.to_f32()),
                None => self.float(fallback),
            },
            FloatNode::EnchantmentLevel(amount) => {
                Some(amount.calculate(self.inputs.context.enchantment_level.unwrap_or(0)))
            }
        }
    }

    fn int_aggregate(&mut self, op: IntAggregate, inputs: &[IntNode]) -> Option<i32> {
        let mut sum: i64 = 0;
        let mut product: i64 = 1;
        let mut min = i32::MAX;
        let mut max = i32::MIN;
        for input in inputs {
            let value = self.int(input)?;
            sum = sum.saturating_add(i64::from(value));
            product = product.wrapping_mul(i64::from(value));
            min = min.min(value);
            max = max.max(value);
        }
        match op {
            IntAggregate::Add => i32::try_from(sum).ok(),
            IntAggregate::Mul => i32::try_from(product).ok(),
            IntAggregate::Min => Some(min),
            IntAggregate::Max => Some(max),
            IntAggregate::Avg => {
                let count = i64::try_from(inputs.len())
                    .ok()
                    .filter(|&count| count > 0)?;
                i32::try_from(sum / count).ok()
            }
        }
    }

    fn float_aggregate(&mut self, op: FloatAggregate, inputs: &[FloatNode]) -> Option<f32> {
        let mut value = match op {
            FloatAggregate::Add | FloatAggregate::Avg | FloatAggregate::Length => 0.0,
            FloatAggregate::Mul => 1.0,
            FloatAggregate::Min => f32::MAX,
            FloatAggregate::Max => -f32::MAX,
        };
        for input in inputs {
            let v = self.float(input)?;
            value = match op {
                FloatAggregate::Add | FloatAggregate::Avg => value + v,
                FloatAggregate::Mul => value * v,
                FloatAggregate::Min => min_f32(value, v),
                FloatAggregate::Max => max_f32(value, v),
                FloatAggregate::Length => value + v * v,
            };
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "toF32 of the number of elements"
        )]
        Some(match op {
            FloatAggregate::Avg => value / inputs.len() as f32,
            FloatAggregate::Length => value.sqrt(),
            FloatAggregate::Add
            | FloatAggregate::Mul
            | FloatAggregate::Min
            | FloatAggregate::Max => value,
        })
    }

    fn test(&mut self, condition: &Condition) -> bool {
        condition.test(self.inputs, self.random, self.scratch)
    }

    fn conditional<'n, N>(
        &mut self,
        condition: &Condition,
        on_true: &'n N,
        on_false: &'n N,
    ) -> &'n N {
        match self.test(condition) {
            true => on_true,
            false => on_false,
        }
    }

    fn dispatch<'n, N>(&mut self, cases: &'n [DispatchCase<N>], default: &'n N) -> &'n N {
        cases
            .iter()
            .find(|case| self.test(&case.condition))
            .map_or(default, |case| &case.value)
    }

    fn weighted<'n, N>(&mut self, total: i32, entries: &'n [WeightedEntry<N>]) -> Option<&'n N> {
        let mut k = draw::bounded(self.random, i64::from(total))?;
        entries
            .iter()
            .find(|entry| {
                k -= entry.weight;
                k < 0
            })
            .map(|entry| &entry.data)
    }

    fn environment_int(&self, read: &EnvironmentRead<i32>) -> i32 {
        match self.environment_value(read) {
            Some(AttributeValue::Integer(value)) => value,
            _ => read.default,
        }
    }

    #[expect(clippy::cast_precision_loss, reason = "toF32 of an integer attribute")]
    fn environment_float(&self, read: &EnvironmentRead<EnvironmentNumber>) -> f32 {
        match (self.environment_value(read), read.default) {
            (Some(AttributeValue::Integer(value)), EnvironmentNumber::Int(_)) => value as f32,
            (Some(AttributeValue::Float(value)), EnvironmentNumber::Float(_)) => value,
            (_, EnvironmentNumber::Int(default)) => default as f32,
            (_, EnvironmentNumber::Float(default)) => default,
        }
    }

    fn environment_value<T>(&self, read: &EnvironmentRead<T>) -> Option<AttributeValue> {
        self.position(read).and_then(|position| {
            self.inputs
                .world
                .environment_value(read.attribute, position)
        })
    }

    fn position<T>(&self, read: &EnvironmentRead<T>) -> Option<Option<DVec3>> {
        match read.positional {
            true => self.inputs.context.origin.map(Some),
            false => Some(None),
        }
    }

    fn score(&self, target: &ScoreTarget, objective: &str) -> Option<i32> {
        let holder = match target {
            ScoreTarget::Entity(param) => ScoreHolder::Entity(self.inputs.context.entity(*param)?),
            ScoreTarget::Name(name) => ScoreHolder::Name(name),
        };
        self.inputs.world.score(holder, objective)
    }
}

fn from_float(a: f32) -> Option<i32> {
    let truncated = a.trunc();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is in the i32 range"
    )]
    (a.is_finite() && (-2_147_483_648.0..2_147_483_648.0).contains(&truncated))
        .then_some(truncated as i32)
}

fn int_binary(op: IntBinary, a: i32, b: i32) -> Option<i32> {
    match op {
        IntBinary::Sub => a.checked_sub(b),
        IntBinary::Mod => (b != 0).then(|| a.wrapping_rem(b)),
        IntBinary::FloorMod => (b != 0).then(|| floor_mod(a, b)),
        IntBinary::Div => (b != 0).then(|| a.wrapping_div(b)),
        IntBinary::FloorDiv => (b != 0 && !(a == i32::MIN && b == -1)).then(|| floor_div(a, b)),
    }
}

fn int_pow(a: i32, b: i32) -> Option<i32> {
    if a == 0 && b == 0 {
        return None;
    }
    a.checked_pow(u32::try_from(b).ok()?)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "toI32 saturates and toF32 is the nearest f32"
)]
fn float_unary(op: FloatUnary, a: f32) -> f32 {
    match op {
        FloatUnary::Abs => a.abs(),
        FloatUnary::Negate => -a,
        FloatUnary::Floor => a.floor(),
        FloatUnary::Ceil => ceil_to_i32(a) as f32,
        FloatUnary::Round => (f64::from(a) + 0.5).floor() as i32 as f32,
        FloatUnary::Truncate => match a > 0.0 {
            true => a.floor(),
            false => a.ceil(),
        },
        FloatUnary::Sqrt => a.sqrt(),
        FloatUnary::Sin => sin_nearest(f64::from(a)),
        FloatUnary::Cos => cos_nearest(f64::from(a)),
    }
}
