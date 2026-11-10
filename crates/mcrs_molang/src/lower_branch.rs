//! The lowering of the lazy operators to `If`, with the merge of temporary variables.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::mem;

use hashbrown::HashMap;

use crate::diagnostic::{ErrorKind, Span};
use crate::ir::{Op, Region, ValueData, ValueId};
use crate::lower::{Lowered, Lowering, Temp};
use crate::math::UnaryFn;
use crate::query::QueryHost;
use crate::syntax::Expr;
use crate::value::{Rate, ValueType};

/// One result of an `If`: the value that each branch yields.
struct Merge {
    then: ValueId,
    otherwise: ValueId,
    value_type: ValueType,
}

enum TempMerge {
    /// Both branches leave the same value.
    Same(Temp),
    /// The `If` gives the value and its missing flag as two results.
    Merged(ValueType),
}

/// The value and the temporaries at the end of one branch.
struct BranchEnd {
    result: Lowered,
    temps: HashMap<Box<str>, Temp>,
    terminated: bool,
    region: Region,
}

impl<H: QueryHost> Lowering<'_, H> {
    pub(crate) fn and(&mut self, left: &Expr, right: &Expr, span: Span) -> Lowered {
        let condition = self.expr(left);
        let Some(condition) = self.expect_number(condition, left.span) else {
            return self.poisoned_zero(span);
        };
        self.branches(
            condition,
            span,
            |this| {
                let operand = this.expr(right);
                let Some(operand) = this.expect_number(operand, right.span) else {
                    return this.poisoned_zero(span);
                };
                let id = this.emit(Op::Unary(UnaryFn::Truthy, operand), ValueType::Number, span);
                this.plain(id, ValueType::Number)
            },
            |this| this.number(0.0, span),
        )
    }

    pub(crate) fn or(&mut self, left: &Expr, right: &Expr, span: Span) -> Lowered {
        let condition = self.expr(left);
        let Some(condition) = self.expect_number(condition, left.span) else {
            return self.poisoned_zero(span);
        };
        self.branches(
            condition,
            span,
            |this| this.number(1.0, span),
            |this| {
                let operand = this.expr(right);
                let Some(operand) = this.expect_number(operand, right.span) else {
                    return this.poisoned_zero(span);
                };
                let id = this.emit(Op::Unary(UnaryFn::Truthy, operand), ValueType::Number, span);
                this.plain(id, ValueType::Number)
            },
        )
    }

    pub(crate) fn conditional(
        &mut self,
        condition: &Expr,
        then: &Expr,
        otherwise: Option<&Expr>,
        span: Span,
    ) -> Lowered {
        let test = self.expr(condition);
        let Some(test) = self.expect_number(test, condition.span) else {
            return self.poisoned_zero(span);
        };
        self.branches(
            test,
            span,
            |this| this.expr(then),
            |this| match otherwise {
                Some(otherwise) => this.expr(otherwise),
                None => this.number(0.0, span),
            },
        )
    }

    pub(crate) fn coalesce(&mut self, left: &Expr, right: &Expr, span: Span) -> Lowered {
        let outer = self.coalesce_depth > 0;
        let saved = outer.then(|| self.emit(Op::MissTake, ValueType::Number, span));
        let checks = self.miss_checks;
        self.coalesce_depth += 1;
        let value = self.expr(left);
        self.coalesce_depth -= 1;
        if value.value_type == ValueType::Resource && !value.poisoned {
            return self.error(
                ErrorKind::StaticType,
                span,
                "`??` does not apply to a resource",
            );
        }
        if self.miss_checks == checks {
            if let Some(saved) = saved
                && let Some(region) = self.regions.last_mut()
            {
                region.insts.retain(|id| *id != saved);
            }
            self.discarded(|this| {
                this.expr(right);
            });
            return value;
        }
        let missing = self.emit(Op::MissTake, ValueType::Number, span);
        if let Some(saved) = saved {
            self.emit(Op::MissOr(saved), ValueType::Number, span);
        }
        let mut chosen_left = value;
        chosen_left.missing = None;
        chosen_left.binding = None;
        self.branches(missing, span, |this| this.expr(right), |_| chosen_left)
    }

    /// Lowers a two-way branch on `condition` to an `If`, and merges the results and the
    /// temporary variables of the two branches.
    pub(crate) fn branches(
        &mut self,
        condition: ValueId,
        span: Span,
        then: impl FnOnce(&mut Self) -> Lowered,
        otherwise: impl FnOnce(&mut Self) -> Lowered,
    ) -> Lowered {
        let before = self.temps.clone();
        let then_end = self.branch(then);
        self.temps = before.clone();
        let else_end = self.branch(otherwise);
        let (a, b) = (then_end.result, else_end.result);
        let poisoned = a.poisoned || b.poisoned;
        if a.value_type != b.value_type && !poisoned {
            self.error(
                ErrorKind::StaticType,
                span,
                "the branches give values of different types",
            );
        }
        let mut merges = alloc::vec![Merge {
            then: a.value,
            otherwise: b.value,
            value_type: a.value_type,
        }];
        let has_missing = a.missing.is_some() || b.missing.is_some();
        if has_missing {
            let then = self.flag_or_zero(a.missing);
            let otherwise = self.flag_or_zero(b.missing);
            merges.push(Merge {
                then,
                otherwise,
                value_type: ValueType::Number,
            });
        }
        let temps = self.merge_temps(&then_end.temps, &else_end.temps, span, &mut merges);
        let phis = self.emit_if(condition, then_end.region, else_end.region, &merges, span);

        let mut phi = phis.into_iter();
        let value = phi.next().unwrap_or(condition);
        let missing = if has_missing { phi.next() } else { None };
        self.temps = before;
        for (name, merged) in temps {
            let temp = match merged {
                TempMerge::Same(temp) => temp,
                TempMerge::Merged(value_type) => match (phi.next(), phi.next()) {
                    (Some(value), Some(missing)) => Temp {
                        value,
                        value_type,
                        missing,
                    },
                    _ => continue,
                },
            };
            self.temps.insert(name, temp);
        }
        self.terminated = then_end.terminated && else_end.terminated;
        Lowered {
            value,
            value_type: a.value_type,
            missing,
            binding: None,
            poisoned,
        }
    }

    fn flag_or_zero(&mut self, flag: Option<ValueId>) -> ValueId {
        match flag {
            Some(flag) => flag,
            None => self.constant_id(0.0),
        }
    }

    /// Adds a merge for each temporary variable that the branches leave with different values.
    fn merge_temps(
        &mut self,
        then: &HashMap<Box<str>, Temp>,
        otherwise: &HashMap<Box<str>, Temp>,
        span: Span,
        merges: &mut Vec<Merge>,
    ) -> Vec<(Box<str>, TempMerge)> {
        let mut names: Vec<&Box<str>> = then.keys().chain(otherwise.keys()).collect();
        names.sort();
        names.dedup();
        let mut temps = Vec::with_capacity(names.len());
        for name in names {
            let (then_temp, else_temp) = (then.get(name).copied(), otherwise.get(name).copied());
            if then_temp == else_temp {
                if let Some(temp) = then_temp {
                    temps.push((name.clone(), TempMerge::Same(temp)));
                }
                continue;
            }
            let value_type = then_temp
                .or(else_temp)
                .map_or(ValueType::Number, |temp| temp.value_type);
            let then_temp = self.temp_or_missing(then_temp, value_type);
            let else_temp = self.temp_or_missing(else_temp, value_type);
            if then_temp.value_type != else_temp.value_type {
                self.error(
                    ErrorKind::StaticType,
                    span,
                    format!("`temp.{name}` holds values of different types in the branches"),
                );
                continue;
            }
            merges.push(Merge {
                then: then_temp.value,
                otherwise: else_temp.value,
                value_type,
            });
            merges.push(Merge {
                then: then_temp.missing,
                otherwise: else_temp.missing,
                value_type: ValueType::Number,
            });
            temps.push((name.clone(), TempMerge::Merged(value_type)));
        }
        temps
    }

    /// Emits the `If` with one result for each merge, and returns the results.
    fn emit_if(
        &mut self,
        condition: ValueId,
        mut then: Region,
        mut otherwise: Region,
        merges: &[Merge],
        span: Span,
    ) -> Vec<ValueId> {
        then.yields.extend(merges.iter().map(|merge| merge.then));
        otherwise
            .yields
            .extend(merges.iter().map(|merge| merge.otherwise));
        let rate = then
            .yields
            .iter()
            .chain(&otherwise.yields)
            .chain([&condition])
            .filter_map(|id| self.values.get(id.index()))
            .map(|data| data.rate)
            .max()
            .unwrap_or(Rate::Constant);
        let mut results = Vec::with_capacity(merges.len());
        for merge in merges {
            results.push(self.values_len());
            self.values.push(ValueData {
                op: Op::Phi,
                value_type: merge.value_type,
                rate,
                span,
            });
        }
        let value_type = merges
            .first()
            .map_or(ValueType::Number, |merge| merge.value_type);
        self.emit(
            Op::If {
                condition,
                then,
                otherwise,
                results: results.clone(),
            },
            value_type,
            span,
        );
        results
    }

    fn values_len(&self) -> ValueId {
        ValueId(u32::try_from(self.values.len()).unwrap_or(u32::MAX))
    }

    fn temp_or_missing(&mut self, temp: Option<Temp>, value_type: ValueType) -> Temp {
        if let Some(temp) = temp {
            return temp;
        }
        let value = self.zero_of(value_type);
        let missing = self.constant_id(1.0);
        Temp {
            value,
            value_type,
            missing,
        }
    }

    fn branch(&mut self, lower: impl FnOnce(&mut Self) -> Lowered) -> BranchEnd {
        self.regions.push(Region::default());
        let terminated = mem::replace(&mut self.terminated, false);
        let result = lower(self);
        let branch_terminated = mem::replace(&mut self.terminated, terminated);
        let region = self.regions.pop().unwrap_or_default();
        BranchEnd {
            result,
            temps: self.temps.clone(),
            terminated: branch_terminated,
            region,
        }
    }
}
