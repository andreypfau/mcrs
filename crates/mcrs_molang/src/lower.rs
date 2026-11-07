use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::compile::CompileOptions;
use crate::diagnostic::{Diagnostic, ErrorKind, Span, Warning};
use crate::ir::{Function, Op, Region, ValueData, ValueId};
use crate::layout::VariableLayout;
use crate::lower_call::literal_of;
use crate::math::{BinaryFn, UnaryFn};
use crate::query::{QueryBinding, QueryHost};
use crate::syntax::{
    BinaryOp, Body, Expr, ExprKind, Stmt, StmtKind, Syntax, UnaryOp, VariableSpace,
};
use crate::value::{Rate, ValueType};

/// A lowered operand.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lowered {
    pub(crate) value: ValueId,
    pub(crate) value_type: ValueType,
    /// `1.0` when the value is a read of a missing value; `None` when it never is.
    pub(crate) missing: Option<ValueId>,
    /// The query that gives the value directly, for the comparison check of the host.
    pub(crate) binding: Option<QueryBinding>,
    /// An error is already reported for the operand; type checks skip it.
    pub(crate) poisoned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Temp {
    pub(crate) value: ValueId,
    pub(crate) value_type: ValueType,
    pub(crate) missing: ValueId,
}

pub(crate) struct LowerOutput {
    pub function: Function,
    pub warnings: Vec<Warning>,
}

pub(crate) fn lower<H: QueryHost>(
    syntax: &Syntax,
    layout: &mut VariableLayout,
    host: &mut H,
    options: &CompileOptions<'_>,
) -> Result<LowerOutput, Vec<Diagnostic>> {
    let mut lowering = Lowering {
        host,
        layout,
        options,
        values: Vec::new(),
        regions: alloc::vec![Region::default()],
        tables: Vec::new(),
        diagnostics: Vec::new(),
        warnings: Vec::new(),
        temps: HashMap::new(),
        coalesce_depth: 0,
        miss_checks: 0,
        output: None,
        terminated: false,
    };
    match &syntax.body {
        Body::Simple(expr) => {
            let result = lowering.expr(expr);
            lowering.ret(result, expr.span);
        }
        Body::Complex(statements) => {
            lowering.statements(statements);
            if !lowering.terminated {
                let zero = lowering.number(0.0, Span::default());
                lowering.ret(zero, Span::default());
            }
        }
    }
    if !lowering.diagnostics.is_empty() {
        return Err(lowering.diagnostics);
    }
    let body = lowering.regions.pop().unwrap_or_default();
    Ok(LowerOutput {
        function: Function {
            values: lowering.values,
            body,
            tables: lowering.tables,
            output: lowering.output.unwrap_or(ValueType::Number),
        },
        warnings: lowering.warnings,
    })
}

pub(crate) struct Lowering<'a, H> {
    pub(crate) host: &'a mut H,
    pub(crate) layout: &'a mut VariableLayout,
    pub(crate) options: &'a CompileOptions<'a>,
    pub(crate) values: Vec<ValueData>,
    pub(crate) regions: Vec<Region>,
    pub(crate) tables: Vec<Vec<u64>>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) warnings: Vec<Warning>,
    pub(crate) temps: HashMap<Box<str>, Temp>,
    pub(crate) coalesce_depth: u32,
    pub(crate) miss_checks: u32,
    pub(crate) output: Option<ValueType>,
    pub(crate) terminated: bool,
}

impl<H: QueryHost> Lowering<'_, H> {
    pub(crate) fn error(
        &mut self,
        kind: ErrorKind,
        span: Span,
        message: impl Into<String>,
    ) -> Lowered {
        self.diagnostics.push(Diagnostic::new(kind, span, message));
        let mut zero = self.number(0.0, span);
        zero.poisoned = true;
        zero
    }

    fn rate_of(&self, op: &Op) -> Rate {
        op.operands()
            .as_slice()
            .iter()
            .filter_map(|id| self.values.get(id.index()))
            .map(|data| data.rate)
            .max()
            .unwrap_or(Rate::Constant)
    }

    pub(crate) fn push(
        &mut self,
        op: Op,
        value_type: ValueType,
        rate: Rate,
        span: Span,
    ) -> ValueId {
        let id = ValueId(u32::try_from(self.values.len()).unwrap_or(u32::MAX));
        self.values.push(ValueData {
            op,
            value_type,
            rate,
            span,
        });
        if let Some(region) = self.regions.last_mut() {
            region.insts.push(id);
        }
        id
    }

    pub(crate) fn emit(&mut self, op: Op, value_type: ValueType, span: Span) -> ValueId {
        let rate = self.rate_of(&op);
        self.push(op, value_type, rate, span)
    }

    pub(crate) fn plain(&self, value: ValueId, value_type: ValueType) -> Lowered {
        Lowered {
            value,
            value_type,
            missing: None,
            binding: None,
            poisoned: false,
        }
    }

    pub(crate) fn number(&mut self, value: f32, span: Span) -> Lowered {
        let id = self.push(Op::Const(value), ValueType::Number, Rate::Constant, span);
        self.plain(id, ValueType::Number)
    }

    pub(crate) fn constant_id(&mut self, value: f32) -> ValueId {
        self.push(
            Op::Const(value),
            ValueType::Number,
            Rate::Constant,
            Span::default(),
        )
    }

    pub(crate) fn zero_of(&mut self, value_type: ValueType) -> ValueId {
        if value_type.is_bits() {
            self.push(
                Op::ConstBits(0),
                value_type,
                Rate::Constant,
                Span::default(),
            )
        } else {
            self.constant_id(0.0)
        }
    }

    fn ret(&mut self, result: Lowered, span: Span) {
        self.unify_output(result, span);
        self.emit(Op::Return(result.value), result.value_type, span);
        self.terminated = true;
    }

    fn unify_output(&mut self, result: Lowered, span: Span) {
        if result.poisoned {
            return;
        }
        match self.output {
            None => self.output = Some(result.value_type),
            Some(output) if output == result.value_type => {}
            Some(_) => {
                self.error(
                    ErrorKind::StaticType,
                    span,
                    "the results of the expression have different types",
                );
            }
        }
    }

    pub(crate) fn statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            if self.terminated {
                self.discarded(|this| this.statement(statement));
            } else {
                self.statement(statement);
            }
        }
    }

    /// Lowers code that never runs, so that its errors are still reported.
    pub(crate) fn discarded(&mut self, lower: impl FnOnce(&mut Self)) {
        let temps = self.temps.clone();
        let terminated = self.terminated;
        let output = self.output;
        self.regions.push(Region::default());
        self.terminated = false;
        lower(self);
        self.regions.pop();
        self.temps = temps;
        self.terminated = terminated;
        self.output = output.or(self.output);
    }

    fn statement(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Empty => {}
            StmtKind::Expr(expr) => {
                self.expr(expr);
            }
            StmtKind::Assign(target, value) => self.assign(target, value, statement.span),
            StmtKind::Return(expr) => {
                let result = self.expr(expr);
                self.ret(result, statement.span);
            }
            StmtKind::Block(statements) => self.statements(statements),
            StmtKind::Break | StmtKind::Continue => {
                self.error(
                    ErrorKind::Misplaced,
                    statement.span,
                    "`break` and `continue` need a loop",
                );
            }
            StmtKind::Loop(..) => {
                self.error(
                    ErrorKind::Unsupported,
                    statement.span,
                    "`loop` is not supported",
                );
            }
            StmtKind::ForEach(..) => {
                self.error(
                    ErrorKind::Unsupported,
                    statement.span,
                    "`for_each` is not supported",
                );
            }
        }
    }

    pub(crate) fn assign(&mut self, target: &Expr, value: &Expr, span: Span) {
        match &target.kind {
            ExprKind::Variable {
                space: VariableSpace::Variable,
                path,
            } => {
                if path.len() > 1 {
                    self.error(
                        ErrorKind::Unsupported,
                        target.span,
                        "struct variables are not supported",
                    );
                    return;
                }
                let name = path.first().map_or("", |name| &**name);
                let result = self.expr(value);
                if result.poisoned {
                    return;
                }
                if result.value_type == ValueType::Resource {
                    self.error(
                        ErrorKind::StaticType,
                        value.span,
                        "a resource cannot be stored in a variable",
                    );
                    return;
                }
                let slot = match self.layout.declare(name, result.value_type) {
                    Ok(slot) => slot,
                    Err(_) => {
                        self.error(
                            ErrorKind::StaticType,
                            span,
                            format!("`variable.{name}` holds a value of another type"),
                        );
                        return;
                    }
                };
                let missing = match result.missing {
                    Some(missing) => missing,
                    None => self.constant_id(0.0),
                };
                self.emit(
                    Op::StoreVar {
                        slot,
                        value: result.value,
                        missing,
                    },
                    result.value_type,
                    span,
                );
            }
            ExprKind::Variable {
                space: VariableSpace::Temp,
                path,
            } => {
                if path.len() > 1 {
                    self.error(
                        ErrorKind::TempStruct,
                        target.span,
                        "a temporary variable has one name",
                    );
                    return;
                }
                let name = path.first().map_or("", |name| &**name);
                let result = self.expr(value);
                if result.value_type == ValueType::Resource && !result.poisoned {
                    self.error(
                        ErrorKind::StaticType,
                        value.span,
                        "a resource cannot be stored in a variable",
                    );
                    return;
                }
                let missing = match result.missing {
                    Some(missing) => missing,
                    None => self.constant_id(0.0),
                };
                self.temps.insert(
                    name.into(),
                    Temp {
                        value: result.value,
                        value_type: result.value_type,
                        missing,
                    },
                );
            }
            _ => {
                self.error(
                    ErrorKind::Unsupported,
                    target.span,
                    "an assignment through `->` is not supported",
                );
            }
        }
    }

    pub(crate) fn expr(&mut self, expr: &Expr) -> Lowered {
        let span = expr.span;
        match &expr.kind {
            ExprKind::Number(value) => self.number(*value, span),
            ExprKind::Bool(value) => self.number(if *value { 1.0 } else { 0.0 }, span),
            ExprKind::String(text) => {
                let id = self.push(
                    Op::ConstBits(crate::value::string_hash(text)),
                    ValueType::String,
                    Rate::Constant,
                    span,
                );
                self.plain(id, ValueType::String)
            }
            ExprKind::This => {
                let id = self.push(Op::This, ValueType::Number, Rate::AnimationFrame, span);
                self.plain(id, ValueType::Number)
            }
            ExprKind::Unary(op, operand) => {
                let operand = self.expr(operand);
                let Some(operand) = self.expect_number(operand, span) else {
                    return self.poisoned_zero(span);
                };
                let function = match op {
                    UnaryOp::Not => UnaryFn::Not,
                    UnaryOp::Negate => UnaryFn::Negate,
                };
                let id = self.emit(Op::Unary(function, operand), ValueType::Number, span);
                self.plain(id, ValueType::Number)
            }
            ExprKind::Binary(BinaryOp::And, left, right) => self.and(left, right, span),
            ExprKind::Binary(BinaryOp::Or, left, right) => self.or(left, right, span),
            ExprKind::Binary(op, left, right) => self.binary(*op, left, right, span),
            ExprKind::Conditional {
                condition,
                then,
                otherwise,
            } => self.conditional(condition, then, otherwise.as_deref(), span),
            ExprKind::Coalesce(left, right) => self.coalesce(left, right, span),
            ExprKind::Block(statements) => {
                self.statements(statements);
                self.number(0.0, span)
            }
            ExprKind::AssignGroup(target, value) => {
                self.assign(target, value, span);
                self.number(0.0, span)
            }
            ExprKind::Query {
                name,
                arguments,
                path,
            } => self.query(name, arguments.as_deref(), path, span),
            ExprKind::Math { name, arguments } => self.math(name, arguments.as_deref(), span),
            ExprKind::Variable { space, path } => self.variable(*space, path, span),
            ExprKind::Resource { kind, name } => {
                let Some(resources) = self.options.resources else {
                    return self.error(
                        ErrorKind::Misplaced,
                        span,
                        "a resource outside a render controller",
                    );
                };
                let Some(resource) = resources.resource(*kind, name) else {
                    return self.error(
                        ErrorKind::UnknownResource,
                        span,
                        format!("no resource with the short name `{name}`"),
                    );
                };
                let id = self.push(
                    Op::ConstBits(resource.to_bits()),
                    ValueType::Resource,
                    Rate::Load,
                    span,
                );
                self.plain(id, ValueType::Resource)
            }
            ExprKind::Array(_) => {
                self.error(ErrorKind::StaticType, span, "an array needs an index")
            }
            ExprKind::Index(array, index) => self.index(array, index, span),
            ExprKind::Arrow(..) => {
                self.error(ErrorKind::Unsupported, span, "`->` is not supported")
            }
        }
    }

    pub(crate) fn poisoned_zero(&mut self, span: Span) -> Lowered {
        let mut zero = self.number(0.0, span);
        zero.poisoned = true;
        zero
    }

    pub(crate) fn expect_number(&mut self, operand: Lowered, span: Span) -> Option<ValueId> {
        if operand.poisoned {
            return None;
        }
        if operand.value_type == ValueType::Number {
            return Some(operand.value);
        }
        self.error(
            ErrorKind::StaticType,
            span,
            "the operator takes numbers only",
        );
        None
    }

    fn binary(&mut self, op: BinaryOp, left_expr: &Expr, right_expr: &Expr, span: Span) -> Lowered {
        let left = self.expr(left_expr);
        let right = self.expr(right_expr);
        if left.poisoned || right.poisoned {
            return self.poisoned_zero(span);
        }
        let function = match op {
            BinaryOp::Add => BinaryFn::Add,
            BinaryOp::Subtract => BinaryFn::Subtract,
            BinaryOp::Multiply => BinaryFn::Multiply,
            BinaryOp::Divide => BinaryFn::Divide,
            BinaryOp::Less => BinaryFn::Less,
            BinaryOp::LessEqual => BinaryFn::LessEqual,
            BinaryOp::Greater => BinaryFn::Greater,
            BinaryOp::GreaterEqual => BinaryFn::GreaterEqual,
            BinaryOp::Equal | BinaryOp::NotEqual => {
                return self.equality(
                    op == BinaryOp::NotEqual,
                    left,
                    right,
                    left_expr,
                    right_expr,
                    span,
                );
            }
            BinaryOp::And | BinaryOp::Or => BinaryFn::BothTruthy,
        };
        if left.value_type != ValueType::Number || right.value_type != ValueType::Number {
            return self.error(
                ErrorKind::StaticType,
                span,
                "the operator takes numbers only",
            );
        }
        let id = self.emit(
            Op::Binary(function, left.value, right.value),
            ValueType::Number,
            span,
        );
        self.plain(id, ValueType::Number)
    }

    fn equality(
        &mut self,
        negate: bool,
        left: Lowered,
        right: Lowered,
        left_expr: &Expr,
        right_expr: &Expr,
        span: Span,
    ) -> Lowered {
        for (operand, other) in [(left, right_expr), (right, left_expr)] {
            if let (Some(binding), Some(literal)) = (operand.binding, literal_of(other))
                && let Err(error) = self.host.check_comparison(binding, literal)
            {
                return self.error(error.kind, span, error.message);
            }
        }
        let id = match (left.value_type, right.value_type) {
            (ValueType::Number, ValueType::Number) => {
                let function = if negate {
                    BinaryFn::NotEqual
                } else {
                    BinaryFn::Equal
                };
                self.emit(
                    Op::Binary(function, left.value, right.value),
                    ValueType::Number,
                    span,
                )
            }
            (ValueType::String, ValueType::String) => self.emit(
                Op::CompareBits {
                    negate,
                    a: left.value,
                    b: right.value,
                },
                ValueType::Number,
                span,
            ),
            (ValueType::String, ValueType::Number) | (ValueType::Number, ValueType::String) => {
                self.constant_id(if negate { 1.0 } else { 0.0 })
            }
            _ => {
                return self.error(ErrorKind::StaticType, span, "a resource cannot be compared");
            }
        };
        self.plain(id, ValueType::Number)
    }

    pub(crate) fn check_missing(&mut self, missing: ValueId, span: Span) {
        let never = matches!(
            self.values.get(missing.index()).map(|data| &data.op),
            Some(Op::Const(value)) if value.to_bits() == 0
        );
        if self.coalesce_depth > 0 && !never {
            self.miss_checks += 1;
            self.emit(Op::MissOr(missing), ValueType::Number, span);
        }
    }

    pub(crate) fn missing_value(&mut self, span: Span) -> Lowered {
        let value = self.constant_id(0.0);
        let missing = self.constant_id(1.0);
        self.check_missing(missing, span);
        Lowered {
            value,
            value_type: ValueType::Number,
            missing: Some(missing),
            binding: None,
            poisoned: false,
        }
    }

    fn variable(&mut self, space: VariableSpace, path: &[Box<str>], span: Span) -> Lowered {
        match space {
            VariableSpace::Variable => {
                if path.len() > 1 {
                    return self.error(
                        ErrorKind::Unsupported,
                        span,
                        "struct variables are not supported",
                    );
                }
                let name = path.first().map_or("", |name| &**name);
                let (slot, value_type) = match self.layout.slot(name) {
                    Some(slot) => (
                        slot,
                        self.layout.value_type(name).unwrap_or(ValueType::Number),
                    ),
                    None => match self.layout.declare(name, ValueType::Number) {
                        Ok(slot) => (slot, ValueType::Number),
                        Err(_) => {
                            return self.error(ErrorKind::StaticType, span, "too many variables");
                        }
                    },
                };
                let value = self.push(Op::LoadVar(slot), value_type, Rate::EntityFrame, span);
                let missing = self.push(
                    Op::VarMissing(slot),
                    ValueType::Number,
                    Rate::EntityFrame,
                    span,
                );
                self.check_missing(missing, span);
                Lowered {
                    value,
                    value_type,
                    missing: Some(missing),
                    binding: None,
                    poisoned: false,
                }
            }
            VariableSpace::Temp => {
                if path.len() > 1 {
                    return self.error(
                        ErrorKind::TempStruct,
                        span,
                        "a temporary variable has one name",
                    );
                }
                let name = path.first().map_or("", |name| &**name);
                let Some(temp) = self.temps.get(name).copied() else {
                    return self.missing_value(span);
                };
                self.check_missing(temp.missing, span);
                Lowered {
                    value: temp.value,
                    value_type: temp.value_type,
                    missing: Some(temp.missing),
                    binding: None,
                    poisoned: false,
                }
            }
            VariableSpace::Context => {
                let name = path.join(".");
                match self.host.resolve_context(&name) {
                    Some(binding) => self.bound_query(binding, span),
                    None => self.missing_value(span),
                }
            }
        }
    }
}
