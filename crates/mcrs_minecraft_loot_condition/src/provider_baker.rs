use core::mem;

use mcrs_minecraft_loot::loot::{ContextFloatProvider, ContextIntProvider};
use mcrs_minecraft_registries::Registry;
use mcrs_minecraft_resource::{Registered, RegistryValue, ResourceLocationBuf, TagKey};

use crate::condition::Condition;
use crate::condition_baker::{ConditionBaker, ProviderScope};
use crate::condition_file::ConditionFile;
use crate::condition_problem::ConditionProblem;
use crate::context_params::ContextParams;
use crate::dispatch_case::DispatchCase;
use crate::dispatch_case_file::DispatchCaseFile;
use crate::distribution_file::DistributionFile;
use crate::entry_kind::EntryKind;
use crate::float_provider::{FloatAggregate, FloatBinary, FloatNode, FloatProvider, FloatUnary};
use crate::float_provider_file::{FloatProviderFile, TypedFloat};
use crate::int_provider::{IntAggregate, IntBinary, IntNode, IntProvider, IntUnary};
use crate::int_provider_file::{IntProviderFile, TypedInt};
use crate::named_source::{Dependency, NamedSource};
use crate::provider_form::ProviderForm;
use crate::provider_set_file::{ProviderSetFile, ProviderTag};
use crate::score_target::ScoreTarget;
use crate::score_target_file::{ScoreTargetFile, TypedScoreTarget};
use crate::weighted_entry::WeightedEntry;

pub(crate) const CHAIN_LIMIT: usize = 64;

impl<'a> ConditionBaker<'a> {
    pub fn int_provider(&mut self, file: &IntProviderFile) -> IntProvider {
        IntProvider(self.in_scope(|baker| baker.int_node(file)))
    }

    pub fn float_provider(&mut self, file: &FloatProviderFile) -> FloatProvider {
        FloatProvider(self.in_scope(|baker| baker.float_node(file)))
    }

    pub(crate) fn condition_int(&mut self, part: &str, file: &IntProviderFile) -> IntProvider {
        match self.scope {
            ProviderScope::Inside => IntProvider(*self.int_at(part, file)),
            ProviderScope::Outside => self.int_provider(file),
        }
    }

    pub(crate) fn condition_float(
        &mut self,
        part: &str,
        file: &FloatProviderFile,
    ) -> FloatProvider {
        match self.scope {
            ProviderScope::Inside => FloatProvider(*self.float_at(part, file)),
            ProviderScope::Outside => self.float_provider(file),
        }
    }

    fn in_scope<N>(&mut self, bake: impl FnOnce(&mut Self) -> N) -> N {
        let scope = mem::replace(&mut self.scope, ProviderScope::Inside);
        let baked = bake(self);
        self.scope = scope;
        baked
    }

    fn int_node(&mut self, file: &IntProviderFile) -> IntNode {
        match &file.0 {
            ProviderForm::Constant(value) => IntNode::Constant(*value),
            ProviderForm::Key(key) => self.named_int(key),
            ProviderForm::Inline(typed) => self.typed_int(typed),
        }
    }

    fn float_node(&mut self, file: &FloatProviderFile) -> FloatNode {
        match &file.0 {
            ProviderForm::Constant(value) => FloatNode::Constant(*value),
            ProviderForm::Key(key) => self.named_float(key),
            ProviderForm::Inline(typed) => self.typed_float(typed),
        }
    }

    fn named_int(&mut self, key: &ResourceLocationBuf) -> IntNode {
        let registry = self.int_providers;
        self.named(key, registry, EntryKind::Int, |index, reads| {
            IntNode::Named { index, reads }
        })
        .unwrap_or(IntNode::Fault)
    }

    fn named_float(&mut self, key: &ResourceLocationBuf) -> FloatNode {
        let registry = self.float_providers;
        self.named(key, registry, EntryKind::Float, |index, reads| {
            FloatNode::Named { index, reads }
        })
        .unwrap_or(FloatNode::Fault)
    }

    fn int_at(&mut self, part: &str, file: &IntProviderFile) -> Box<IntNode> {
        Box::new(match &file.0 {
            ProviderForm::Key(key) => self.named_int(key),
            ProviderForm::Constant(_) | ProviderForm::Inline(_) => self
                .step(part, |baker| baker.int_node(file))
                .unwrap_or(IntNode::Fault),
        })
    }

    fn float_at(&mut self, part: &str, file: &FloatProviderFile) -> Box<FloatNode> {
        Box::new(match &file.0 {
            ProviderForm::Key(key) => self.named_float(key),
            ProviderForm::Constant(_) | ProviderForm::Inline(_) => self
                .step(part, |baker| baker.float_node(file))
                .unwrap_or(FloatNode::Fault),
        })
    }

    fn int_or_zero(&mut self, part: &str, file: Option<&IntProviderFile>) -> Box<IntNode> {
        file.map_or_else(
            || Box::new(IntNode::Constant(0)),
            |file| self.int_at(part, file),
        )
    }

    fn float_or_zero(&mut self, part: &str, file: Option<&FloatProviderFile>) -> Box<FloatNode> {
        file.map_or_else(
            || Box::new(FloatNode::Constant(0.0)),
            |file| self.float_at(part, file),
        )
    }

    fn typed_int(&mut self, typed: &TypedInt) -> IntNode {
        let binary = |baker: &mut Self, op, left, right| {
            IntNode::Binary(op, baker.int_at("left", left), baker.int_at("right", right))
        };
        match typed {
            TypedInt::Constant { value } => IntNode::Constant(value.0),
            TypedInt::Abs { input } => IntNode::Unary(IntUnary::Abs, self.int_at("input", input)),
            TypedInt::Negate { input } => {
                IntNode::Unary(IntUnary::Negate, self.int_at("input", input))
            }
            TypedInt::FromFloat { input } => IntNode::FromFloat(self.float_at("input", input)),
            TypedInt::Sub { left, right } => binary(self, IntBinary::Sub, left, right),
            TypedInt::Mod { left, right } => binary(self, IntBinary::Mod, left, right),
            TypedInt::FloorMod { left, right } => binary(self, IntBinary::FloorMod, left, right),
            TypedInt::Div { left, right } => binary(self, IntBinary::Div, left, right),
            TypedInt::FloorDiv { left, right } => binary(self, IntBinary::FloorDiv, left, right),
            TypedInt::Pow { base, exponent } => {
                IntNode::Pow(self.int_at("base", base), self.int_at("exponent", exponent))
            }
            TypedInt::Uniform { min, max } => {
                IntNode::Uniform(self.int_at("min", min), self.int_at("max", max))
            }
            TypedInt::Add { inputs } => self.int_aggregate(IntAggregate::Add, inputs),
            TypedInt::Mul { inputs } => self.int_aggregate(IntAggregate::Mul, inputs),
            TypedInt::Min { inputs } => self.int_aggregate(IntAggregate::Min, inputs),
            TypedInt::Max { inputs } => self.int_aggregate(IntAggregate::Max, inputs),
            TypedInt::Avg { inputs } => self.int_aggregate(IntAggregate::Avg, inputs),
            TypedInt::Conditional {
                condition,
                on_true,
                on_false,
            } => IntNode::Conditional {
                condition: Box::new(self.inner_condition(condition)),
                on_true: self.int_at("on_true", on_true),
                on_false: self.int_or_zero("on_false", on_false.as_ref()),
            },
            TypedInt::NumberDispatcher { cases, default } => IntNode::Dispatcher {
                cases: self.cases(cases, Self::int_at),
                default: self.int_or_zero("default", default.as_ref()),
            },
            TypedInt::WeightedList { distribution } => IntNode::WeightedList {
                total: distribution.total,
                entries: self.weighted(distribution, Self::int_at),
            },
            TypedInt::EnvironmentAttribute { attribute } => IntNode::Environment(*attribute),
            TypedInt::Storage {
                storage,
                path,
                fallback,
            } => IntNode::Storage {
                storage: storage.clone(),
                path: path.clone(),
                fallback: self.int_or_zero("fallback", fallback.as_ref()),
            },
            TypedInt::Binomial { n, p } => {
                IntNode::Binomial(self.int_at("n", n), self.float_at("p", p))
            }
            TypedInt::Score {
                target,
                score,
                fallback,
            } => IntNode::Score {
                target: score_target(target),
                objective: score.as_str().into(),
                fallback: self.int_or_zero("fallback", fallback.as_ref()),
            },
        }
    }

    fn typed_float(&mut self, typed: &TypedFloat) -> FloatNode {
        let unary =
            |baker: &mut Self, op, input| FloatNode::Unary(op, baker.float_at("input", input));
        let binary = |baker: &mut Self, op, left, right| {
            FloatNode::Binary(
                op,
                baker.float_at("left", left),
                baker.float_at("right", right),
            )
        };
        match typed {
            TypedFloat::Constant { value } => FloatNode::Constant(value.0),
            TypedFloat::Abs { input } => unary(self, FloatUnary::Abs, input),
            TypedFloat::Negate { input } => unary(self, FloatUnary::Negate, input),
            TypedFloat::Floor { input } => unary(self, FloatUnary::Floor, input),
            TypedFloat::Ceil { input } => unary(self, FloatUnary::Ceil, input),
            TypedFloat::Round { input } => unary(self, FloatUnary::Round, input),
            TypedFloat::Truncate { input } => unary(self, FloatUnary::Truncate, input),
            TypedFloat::Sqrt { input } => unary(self, FloatUnary::Sqrt, input),
            TypedFloat::Sin { input } => unary(self, FloatUnary::Sin, input),
            TypedFloat::Cos { input } => unary(self, FloatUnary::Cos, input),
            TypedFloat::FromInt { input } => FloatNode::FromInt(self.int_at("input", input)),
            TypedFloat::Sub { left, right } => binary(self, FloatBinary::Sub, left, right),
            TypedFloat::Div { left, right } => binary(self, FloatBinary::Div, left, right),
            TypedFloat::Mod { left, right } => {
                FloatNode::Mod(self.float_at("left", left), self.float_at("right", right))
            }
            TypedFloat::Pow { base, exponent } => FloatNode::Pow(
                self.float_at("base", base),
                self.float_at("exponent", exponent),
            ),
            TypedFloat::Uniform { min, max } => {
                FloatNode::Uniform(self.float_at("min", min), self.float_at("max", max))
            }
            TypedFloat::Add { inputs } => self.float_aggregate(FloatAggregate::Add, inputs),
            TypedFloat::Mul { inputs } => self.float_aggregate(FloatAggregate::Mul, inputs),
            TypedFloat::Min { inputs } => self.float_aggregate(FloatAggregate::Min, inputs),
            TypedFloat::Max { inputs } => self.float_aggregate(FloatAggregate::Max, inputs),
            TypedFloat::Avg { inputs } => self.float_aggregate(FloatAggregate::Avg, inputs),
            TypedFloat::Length { inputs } => self.float_aggregate(FloatAggregate::Length, inputs),
            TypedFloat::Conditional {
                condition,
                on_true,
                on_false,
            } => FloatNode::Conditional {
                condition: Box::new(self.inner_condition(condition)),
                on_true: self.float_at("on_true", on_true),
                on_false: self.float_or_zero("on_false", on_false.as_ref()),
            },
            TypedFloat::NumberDispatcher { cases, default } => FloatNode::Dispatcher {
                cases: self.cases(cases, Self::float_at),
                default: self.float_or_zero("default", default.as_ref()),
            },
            TypedFloat::WeightedList { distribution } => FloatNode::WeightedList {
                total: distribution.total,
                entries: self.weighted(distribution, Self::float_at),
            },
            TypedFloat::EnvironmentAttribute { attribute } => FloatNode::Environment(*attribute),
            TypedFloat::Storage {
                storage,
                path,
                fallback,
            } => FloatNode::Storage {
                storage: storage.clone(),
                path: path.clone(),
                fallback: self.float_or_zero("fallback", fallback.as_ref()),
            },
            TypedFloat::EnchantmentLevel { amount } => FloatNode::EnchantmentLevel(amount.clone()),
        }
    }

    fn inner_condition(&mut self, file: &ConditionFile) -> Condition {
        Condition(self.test(file))
    }

    fn cases<P, N>(
        &mut self,
        cases: &[DispatchCaseFile<P>],
        bake: fn(&mut Self, &str, &P) -> Box<N>,
    ) -> Box<[DispatchCase<N>]> {
        cases
            .iter()
            .enumerate()
            .map(|(number, case)| DispatchCase {
                condition: self.inner_condition(&case.condition),
                value: *bake(self, &format!("cases[{number}].value"), &case.value),
            })
            .collect()
    }

    fn weighted<P, N>(
        &mut self,
        distribution: &DistributionFile<P>,
        bake: fn(&mut Self, &str, &P) -> Box<N>,
    ) -> Box<[WeightedEntry<N>]> {
        distribution
            .entries
            .iter()
            .enumerate()
            .map(|(number, entry)| WeightedEntry {
                weight: entry.weight,
                data: *bake(self, &format!("distribution[{number}].data"), &entry.data),
            })
            .collect()
    }

    fn int_aggregate(
        &mut self,
        op: IntAggregate,
        inputs: &ProviderSetFile<IntProviderFile>,
    ) -> IntNode {
        let inputs = match inputs {
            ProviderSetFile::Tag(tag) => self
                .tag_members::<ContextIntProvider>(tag)
                .map(|keys| keys.into_iter().map(|key| self.named_int(key)).collect()),
            ProviderSetFile::Many(list) => Some(
                list.iter()
                    .enumerate()
                    .map(|(number, file)| *self.int_at(&format!("inputs[{number}]"), file))
                    .collect(),
            ),
            ProviderSetFile::One(file) => {
                Some(Box::new([*self.int_at("inputs", file)]) as Box<[_]>)
            }
        };
        match self.non_empty(inputs) {
            Some(inputs) => IntNode::Aggregate(op, inputs),
            None => IntNode::Fault,
        }
    }

    fn float_aggregate(
        &mut self,
        op: FloatAggregate,
        inputs: &ProviderSetFile<FloatProviderFile>,
    ) -> FloatNode {
        let inputs = match inputs {
            ProviderSetFile::Tag(tag) => self
                .tag_members::<ContextFloatProvider>(tag)
                .map(|keys| keys.into_iter().map(|key| self.named_float(key)).collect()),
            ProviderSetFile::Many(list) => Some(
                list.iter()
                    .enumerate()
                    .map(|(number, file)| *self.float_at(&format!("inputs[{number}]"), file))
                    .collect(),
            ),
            ProviderSetFile::One(file) => {
                Some(Box::new([*self.float_at("inputs", file)]) as Box<[_]>)
            }
        };
        match self.non_empty(inputs) {
            Some(inputs) => FloatNode::Aggregate(op, inputs),
            None => FloatNode::Fault,
        }
    }

    fn non_empty<N>(&mut self, inputs: Option<Box<[N]>>) -> Option<Box<[N]>> {
        let inputs = inputs?;
        if inputs.is_empty() {
            self.problems
                .push(ConditionProblem::EmptyInputs(self.path(None)));
            return None;
        }
        Some(inputs)
    }

    fn tag_members<R: Registered>(
        &mut self,
        tag: &ProviderTag,
    ) -> Option<Vec<&'a ResourceLocationBuf>> {
        let registries = self.registries;
        let members = registries.tags::<R>().and_then(|tags| {
            let names = registries.entries::<R>()?;
            let members = tags.members(tags.id(TagKey::new(tag.0.as_location()))?);
            members.map(|id| names.name(id)).collect::<Option<Vec<_>>>()
        });
        if members.is_none() {
            self.problems
                .push(ConditionProblem::UnknownProvider(format!("#{}", tag.0)));
        }
        members
    }

    pub(crate) fn named<F: RegistryValue, N>(
        &mut self,
        key: &ResourceLocationBuf,
        registry: Option<&'a Registry<F>>,
        kind: EntryKind,
        reference: fn(usize, ContextParams) -> N,
    ) -> Option<N> {
        let index = registry.and_then(|registry| registry.by_name(key.as_location()));
        let steps = self.chain.len().saturating_add(1);
        let reads = match (index, &mut self.named) {
            (None, _) => None,
            (Some(index), NamedSource::Recording(dependencies)) => {
                dependencies.push(Dependency {
                    kind,
                    index: index.index(),
                    steps,
                });
                Some(ContextParams::EMPTY)
            }
            (Some(index), NamedSource::Baked(entries)) => {
                entries.reach(kind, index.index()).map(|reach| {
                    self.longest = self.longest.max(steps.saturating_add(reach.below));
                    reach.reads
                })
            }
        };
        let Some((index, reads)) = index.zip(reads) else {
            let unknown = match kind {
                EntryKind::Predicate => ConditionProblem::UnknownPredicate,
                EntryKind::Int | EntryKind::Float => ConditionProblem::UnknownProvider,
            };
            self.problems.push(unknown(key.to_string()));
            return None;
        };
        Some(reference(index.index(), reads))
    }

    fn step<N>(&mut self, part: &str, bake: impl FnOnce(&mut Self) -> N) -> Option<N> {
        if self.chain.len() + 1 >= CHAIN_LIMIT {
            let path = self.path(Some(part));
            self.problems.push(ConditionProblem::ChainTooLong(path));
            return None;
        }
        self.chain.push(part.to_owned());
        self.longest = self.longest.max(self.chain.len());
        let baked = bake(self);
        self.chain.pop();
        Some(baked)
    }

    fn path(&self, last: Option<&str>) -> String {
        let steps: Vec<&str> = self.chain.iter().map(String::as_str).chain(last).collect();
        steps.join(" > ")
    }
}

fn score_target(target: &ScoreTargetFile) -> ScoreTarget {
    match target {
        ScoreTargetFile::Bare(target)
        | ScoreTargetFile::Typed(TypedScoreTarget::Context { target }) => {
            ScoreTarget::Entity(target.param())
        }
        ScoreTargetFile::Typed(TypedScoreTarget::Fixed { name }) => {
            ScoreTarget::Name(name.as_str().into())
        }
    }
}
