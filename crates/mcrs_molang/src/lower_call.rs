//! The lowering of query references, `math.` calls and array indices.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use crate::diagnostic::{ErrorKind, Span, Warning, WarningKind};
use crate::ir::{Op, ValueId};
use crate::lower::{Lowered, Lowering};
use crate::math::{BinaryFn, MathName, TernaryFn};
use crate::query::{Argument, Literal, QueryBinding, QueryHost, Resolution, UseContext};
use crate::syntax::{Expr, ExprKind};
use crate::value::{Milestone, Rate, ValueType};

impl<H: QueryHost> Lowering<'_, H> {
    pub(crate) fn bound_query(&mut self, binding: QueryBinding, span: Span) -> Lowered {
        let value = self.push(
            Op::Query(binding.slot),
            binding.value_type,
            binding.rate,
            span,
        );
        let missing = self.push(
            Op::QueryMissing(binding.slot),
            ValueType::Number,
            binding.rate,
            span,
        );
        self.check_missing(missing, span);
        Lowered {
            value,
            value_type: binding.value_type,
            missing: Some(missing),
            binding: Some(binding),
            poisoned: false,
        }
    }

    pub(crate) fn query(
        &mut self,
        name: &str,
        arguments: Option<&[Expr]>,
        path: &[Box<str>],
        span: Span,
    ) -> Lowered {
        if let Some(pure) = PureQuery::lookup(name) {
            return self.pure_query(pure, name, arguments, path, span);
        }
        let mut lowered_arguments = Vec::new();
        for argument in arguments.unwrap_or_default() {
            lowered_arguments.push(match literal_of(argument) {
                Some(literal) => Argument::Literal(literal),
                None => {
                    self.expr(argument);
                    Argument::Dynamic
                }
            });
        }
        let given = arguments.map(|_| lowered_arguments.as_slice());
        let resolution = match self.host.resolve_query(name, given, self.options.milestone) {
            Ok(resolution) => resolution,
            Err(error) => return self.error(error.kind, span, error.message),
        };
        match resolution {
            Resolution::Bound(binding) => {
                if !path.is_empty() {
                    return self.error(
                        ErrorKind::StaticType,
                        span,
                        format!("`query.{name}` has no members"),
                    );
                }
                self.bound_query(binding, span)
            }
            Resolution::Unknown if self.host.use_context() == UseContext::Block => self.error(
                ErrorKind::UnknownQuery,
                span,
                format!("no query `query.{name}` in a block condition"),
            ),
            Resolution::Unknown | Resolution::OutOfContext => {
                let kind = if resolution == Resolution::Unknown {
                    WarningKind::UnknownQuery
                } else {
                    WarningKind::QueryOutOfContext
                };
                if !self
                    .warnings
                    .iter()
                    .any(|w| w.kind == kind && *w.name == *name)
                {
                    self.warnings.push(Warning {
                        kind,
                        span,
                        name: name.into(),
                    });
                }
                self.missing_value(span)
            }
        }
    }

    fn pure_query(
        &mut self,
        pure: PureQuery,
        name: &str,
        arguments: Option<&[Expr]>,
        path: &[Box<str>],
        span: Span,
    ) -> Lowered {
        if self.options.milestone < Milestone::Two {
            return self.error(
                ErrorKind::Unsupported,
                span,
                format!("`query.{name}` belongs to a later milestone"),
            );
        }
        let arguments = arguments.unwrap_or_default();
        if !pure.accepts(arguments.len()) {
            return self.error(
                ErrorKind::Arity,
                span,
                format!("`query.{name}` does not take {} arguments", arguments.len()),
            );
        }
        if !path.is_empty() {
            return self.error(
                ErrorKind::StaticType,
                span,
                format!("`query.{name}` has no members"),
            );
        }
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let lowered = self.expr(argument);
            match self.expect_number(lowered, argument.span) {
                Some(value) => values.push(value),
                None => return self.poisoned_zero(span),
            }
        }
        let Some((&subject, rest)) = values.split_first() else {
            return self.poisoned_zero(span);
        };
        let id = match pure {
            PureQuery::InRange => match rest {
                [lo, hi] => self.emit(
                    Op::Ternary(TernaryFn::InRange, subject, *lo, *hi),
                    ValueType::Number,
                    span,
                ),
                _ => return self.poisoned_zero(span),
            },
            PureQuery::ApproxEq | PureQuery::All | PureQuery::Any => {
                let (test, join) = match pure {
                    PureQuery::ApproxEq => (BinaryFn::ApproxEqual, BinaryFn::BothTruthy),
                    PureQuery::All => (BinaryFn::Equal, BinaryFn::BothTruthy),
                    _ => (BinaryFn::Equal, BinaryFn::EitherTruthy),
                };
                let mut joined: Option<ValueId> = None;
                for value in rest {
                    let tested =
                        self.emit(Op::Binary(test, subject, *value), ValueType::Number, span);
                    joined = Some(match joined {
                        Some(previous) => {
                            self.emit(Op::Binary(join, previous, tested), ValueType::Number, span)
                        }
                        None => tested,
                    });
                }
                match joined {
                    Some(joined) => joined,
                    None => return self.poisoned_zero(span),
                }
            }
        };
        self.plain(id, ValueType::Number)
    }

    pub(crate) fn math(&mut self, name: &str, arguments: Option<&[Expr]>, span: Span) -> Lowered {
        let Some(function) = MathName::lookup(name) else {
            return self.error(
                ErrorKind::UnknownFunction,
                span,
                format!("no function `math.{name}`"),
            );
        };
        if function == MathName::Pi {
            if arguments.is_some() {
                return self.error(ErrorKind::Arity, span, "`math.pi` has no argument list");
            }
            return self.number(core::f32::consts::PI, span);
        }
        let Some(arguments) = arguments else {
            return self.error(
                ErrorKind::Arity,
                span,
                format!("`math.{name}` needs an argument list"),
            );
        };
        if arguments.len() != function.arity() {
            return self.error(
                ErrorKind::Arity,
                span,
                format!(
                    "`math.{name}` takes {} arguments, not {}",
                    function.arity(),
                    arguments.len()
                ),
            );
        }
        if matches!(function, MathName::Random(_)) {
            return self.error(
                ErrorKind::Unsupported,
                span,
                format!("`math.{name}` belongs to a later milestone"),
            );
        }
        if function.milestone() > self.options.milestone {
            return self.error(
                ErrorKind::Unsupported,
                span,
                format!("`math.{name}` belongs to a later milestone"),
            );
        }
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let lowered = self.expr(argument);
            match self.expect_number(lowered, argument.span) {
                Some(value) => values.push(value),
                None => return self.poisoned_zero(span),
            }
        }
        let op = match (function, values.as_slice()) {
            (MathName::Unary(function), [a]) => Op::Unary(function, *a),
            (MathName::Binary(function), [a, b]) => Op::Binary(function, *a, *b),
            (MathName::Ternary(function), [a, b, c]) => Op::Ternary(function, *a, *b, *c),
            _ => return self.poisoned_zero(span),
        };
        let id = self.emit(op, ValueType::Number, span);
        self.plain(id, ValueType::Number)
    }

    pub(crate) fn index(&mut self, array: &Expr, index: &Expr, span: Span) -> Lowered {
        let ExprKind::Array(name) = &array.kind else {
            return self.error(ErrorKind::StaticType, span, "only an array takes an index");
        };
        let Some(resources) = self.options.resources else {
            return self.error(
                ErrorKind::Misplaced,
                span,
                "an array outside a render controller",
            );
        };
        let Some(elements) = resources.array(name) else {
            return self.error(
                ErrorKind::Misplaced,
                span,
                format!("no array `array.{name}`"),
            );
        };
        let raw = elements.raw();
        let value_type = elements.element_type();
        if raw.is_empty() {
            return self.error(
                ErrorKind::StaticType,
                span,
                format!("`array.{name}` is empty"),
            );
        }
        let index = self.expr(index);
        let Some(index) = self.expect_number(index, span) else {
            return self.poisoned_zero(span);
        };
        let table = match u16::try_from(self.tables.len()) {
            Ok(table) => table,
            Err(_) => return self.error(ErrorKind::StaticType, span, "too many arrays"),
        };
        self.tables.push(raw);
        let id = self.emit(Op::Table(table, index), value_type, span);
        let rate = self
            .values
            .get(id.index())
            .map_or(Rate::Load, |data| data.rate.max(Rate::Load));
        if let Some(data) = self.values.get_mut(id.index()) {
            data.rate = rate;
        }
        self.plain(id, value_type)
    }
}

pub(crate) fn literal_of(expr: &Expr) -> Option<Literal<'_>> {
    match &expr.kind {
        ExprKind::Number(value) => Some(Literal::Number(*value)),
        ExprKind::Bool(value) => Some(Literal::Bool(*value)),
        ExprKind::String(text) => Some(Literal::String(text)),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PureQuery {
    InRange,
    ApproxEq,
    All,
    Any,
}

impl PureQuery {
    fn lookup(name: &str) -> Option<Self> {
        match name {
            "in_range" => Some(Self::InRange),
            "approx_eq" => Some(Self::ApproxEq),
            "all" => Some(Self::All),
            "any" => Some(Self::Any),
            _ => None,
        }
    }

    fn accepts(self, count: usize) -> bool {
        match self {
            Self::InRange => count == 3,
            Self::ApproxEq => count >= 2,
            Self::All | Self::Any => count >= 3,
        }
    }
}
