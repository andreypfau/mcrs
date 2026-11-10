use std::collections::{BTreeMap, BTreeSet};

use mcrs_molang::{
    BinaryOp, Body, Expr, ExprKind, ResourceKind, Stmt, StmtKind, Syntax, UnaryOp, VariableSpace,
};

/// A value that an expression reads from outside the run.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Read {
    Query(String),
    Variable(String),
    Context(String),
    This,
    Random,
    Resource,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueryUse {
    pub name: String,
    pub arity: Option<usize>,
    pub literal_args: bool,
    pub via_arrow: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MathUse {
    pub name: String,
    pub arity: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariableWrite {
    pub name: String,
    pub deps: BTreeSet<Read>,
}

/// The facts of one parsed expression that the report aggregates.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub nodes: usize,
    pub statements: usize,
    pub queries: Vec<QueryUse>,
    pub math: Vec<MathUse>,
    pub features: BTreeMap<&'static str, usize>,
    pub reads: BTreeSet<Read>,
    pub writes: Vec<VariableWrite>,
    pub variable_names: BTreeSet<String>,
    pub temp_struct: bool,
}

#[must_use]
pub fn analyse(syntax: &Syntax) -> Facts {
    let mut walker = Walker {
        facts: Facts::default(),
        temps: BTreeMap::new(),
        arrow_depth: 0,
    };
    let control = BTreeSet::new();
    match &syntax.body {
        Body::Simple(expr) => {
            walker.expr(expr, &control);
        }
        Body::Complex(statements) => walker.statements(statements, &control),
    }
    walker.facts
}

struct Walker {
    facts: Facts,
    temps: BTreeMap<String, BTreeSet<Read>>,
    arrow_depth: usize,
}

impl Walker {
    fn feature(&mut self, name: &'static str) {
        *self.facts.features.entry(name).or_default() += 1;
    }

    fn read(&mut self, read: Read) -> BTreeSet<Read> {
        self.facts.reads.insert(read.clone());
        BTreeSet::from([read])
    }

    fn statements(&mut self, statements: &[Stmt], control: &BTreeSet<Read>) {
        for statement in statements {
            self.statement(statement, control);
        }
    }

    fn statement(&mut self, statement: &Stmt, control: &BTreeSet<Read>) {
        if statement.kind == StmtKind::Empty {
            return;
        }
        self.facts.nodes += 1;
        self.facts.statements += 1;
        match &statement.kind {
            StmtKind::Empty => {}
            StmtKind::Return(value) => {
                self.feature("return");
                self.expr(value, control);
            }
            StmtKind::Break => self.feature("break"),
            StmtKind::Continue => self.feature("continue"),
            StmtKind::Assign(target, value) => self.assign(target, value, control),
            StmtKind::Loop(count, body) => {
                self.feature("loop");
                let mut inner = control.clone();
                inner.extend(self.expr(count, control));
                self.statements(body, &inner);
            }
            StmtKind::ForEach(variable, collection, body) => {
                self.feature("for_each");
                let mut inner = control.clone();
                inner.extend(self.expr(collection, control));
                self.write_target(variable, inner.clone());
                self.statements(body, &inner);
            }
            StmtKind::Block(body) => {
                self.feature("block statement");
                self.statements(body, control);
            }
            StmtKind::Expr(expr) => {
                self.expr(expr, control);
            }
        }
    }

    fn assign(&mut self, target: &Expr, value: &Expr, control: &BTreeSet<Read>) {
        let mut deps = self.expr(value, control);
        deps.extend(control.iter().cloned());
        match &target.kind {
            ExprKind::Variable { space, path } => {
                let name = match space {
                    VariableSpace::Variable => {
                        if path.len() > 1 {
                            self.feature("struct write `v.a.b = `");
                        }
                        "assign `v.`"
                    }
                    VariableSpace::Temp => "assign `t.`",
                    VariableSpace::Context => "assign `c.` (read-only)",
                };
                self.feature(name);
            }
            ExprKind::Arrow(arrow_target, _) => {
                self.feature("assign through `->`");
                deps.extend(self.expr(arrow_target, control));
            }
            _ => {}
        }
        self.write_target(target, deps);
    }

    fn write_target(&mut self, target: &Expr, deps: BTreeSet<Read>) {
        let ExprKind::Variable { space, path } = &target.kind else {
            return;
        };
        self.facts.nodes += 1;
        let joined = path.join(".");
        match space {
            VariableSpace::Variable => {
                self.facts.variable_names.insert(joined.clone());
                self.facts.writes.push(VariableWrite { name: joined, deps });
            }
            VariableSpace::Temp => {
                self.facts.temp_struct |= path.len() > 1;
                self.temps.entry(joined).or_default().extend(deps);
            }
            VariableSpace::Context => {}
        }
    }

    fn expr(&mut self, expr: &Expr, control: &BTreeSet<Read>) -> BTreeSet<Read> {
        self.facts.nodes += 1;
        match &expr.kind {
            ExprKind::Number(_) | ExprKind::Bool(_) => BTreeSet::new(),
            ExprKind::String(_) => {
                self.feature("string literal");
                BTreeSet::new()
            }
            ExprKind::This => {
                self.feature("`this`");
                self.read(Read::This)
            }
            ExprKind::Query {
                name, arguments, ..
            } => self.query(name, arguments.as_deref(), control),
            ExprKind::Math { name, arguments } => self.math(name, arguments.as_deref(), control),
            ExprKind::Variable { space, path } => self.path(*space, path),
            ExprKind::Resource { kind, .. } => {
                self.feature(match kind {
                    ResourceKind::Geometry => "`geometry.` reference",
                    ResourceKind::Material => "`material.` reference",
                    ResourceKind::Texture => "`texture.` reference",
                });
                self.read(Read::Resource)
            }
            ExprKind::Array(_) => {
                self.feature("`array.` reference");
                self.read(Read::Resource)
            }
            ExprKind::Unary(op, operand) => {
                self.feature(match op {
                    UnaryOp::Not => "unary `!`",
                    UnaryOp::Negate => "unary `-`",
                });
                self.expr(operand, control)
            }
            ExprKind::Binary(op, lhs, rhs) => self.binary(*op, lhs, rhs, control),
            ExprKind::Coalesce(lhs, rhs) => {
                self.feature("`??`");
                self.string_operand(lhs, rhs, "string operand of another operator");
                let mut reads = self.expr(lhs, control);
                reads.extend(self.expr(rhs, control));
                reads
            }
            ExprKind::Conditional {
                condition,
                then,
                otherwise,
            } => {
                self.feature(if otherwise.is_some() {
                    "`? :`"
                } else {
                    "binary `?`"
                });
                let mut reads = self.expr(condition, control);
                let mut inner = control.clone();
                inner.extend(reads.iter().cloned());
                reads.extend(self.expr(then, &inner));
                if let Some(otherwise) = otherwise {
                    reads.extend(self.expr(otherwise, &inner));
                }
                reads
            }
            ExprKind::Block(body) => {
                self.feature("block branch `? {}`");
                self.statements(body, control);
                BTreeSet::new()
            }
            ExprKind::AssignGroup(target, value) => {
                self.feature("assignment group `(a = b)`");
                self.assign(target, value, control);
                BTreeSet::new()
            }
            ExprKind::Index(target, index) => {
                self.feature("index `[]`");
                let mut reads = self.expr(target, control);
                reads.extend(self.expr(index, control));
                reads
            }
            ExprKind::Arrow(target, reference) => {
                self.feature("`->`");
                let mut reads = self.expr(target, control);
                self.arrow_depth += 1;
                reads.extend(self.expr(reference, control));
                self.arrow_depth -= 1;
                reads
            }
        }
    }

    fn string_operand(&mut self, lhs: &Expr, rhs: &Expr, feature: &'static str) {
        let is_string = |expr: &Expr| matches!(expr.kind, ExprKind::String(_));
        if is_string(lhs) || is_string(rhs) {
            self.feature(feature);
        }
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        control: &BTreeSet<Read>,
    ) -> BTreeSet<Read> {
        self.feature(binary_feature(op));
        let feature = match op {
            BinaryOp::Equal | BinaryOp::NotEqual => "string compare `==`/`!=`",
            _ => "string operand of another operator",
        };
        self.string_operand(lhs, rhs, feature);
        let mut reads = self.expr(lhs, control);
        reads.extend(self.expr(rhs, control));
        reads
    }

    fn arguments(
        &mut self,
        arguments: Option<&[Expr]>,
        control: &BTreeSet<Read>,
    ) -> BTreeSet<Read> {
        let mut reads = BTreeSet::new();
        for argument in arguments.unwrap_or_default() {
            reads.extend(self.expr(argument, control));
        }
        reads
    }

    fn query(
        &mut self,
        name: &str,
        arguments: Option<&[Expr]>,
        control: &BTreeSet<Read>,
    ) -> BTreeSet<Read> {
        let mut reads = self.arguments(arguments, control);
        self.feature("query");
        let literal_args = arguments.unwrap_or_default().iter().all(|argument| {
            matches!(
                argument.kind,
                ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::String(_)
            )
        });
        self.facts.queries.push(QueryUse {
            name: name.to_owned(),
            arity: arguments.map(<[Expr]>::len),
            literal_args,
            via_arrow: self.arrow_depth > 0,
        });
        reads.extend(self.read(Read::Query(name.to_owned())));
        reads
    }

    fn math(
        &mut self,
        name: &str,
        arguments: Option<&[Expr]>,
        control: &BTreeSet<Read>,
    ) -> BTreeSet<Read> {
        let mut reads = self.arguments(arguments, control);
        self.feature("math");
        self.facts.math.push(MathUse {
            name: name.to_owned(),
            arity: arguments.map(<[Expr]>::len),
        });
        if is_random(name) {
            reads.extend(self.read(Read::Random));
        }
        reads
    }

    fn path(&mut self, space: VariableSpace, path: &[Box<str>]) -> BTreeSet<Read> {
        let joined = path.join(".");
        match space {
            VariableSpace::Variable => {
                self.feature(if path.len() > 1 {
                    "read `v.a.b` (struct path)"
                } else {
                    "read `v.`"
                });
                self.facts.variable_names.insert(joined.clone());
                self.read(Read::Variable(joined))
            }
            VariableSpace::Temp => {
                self.feature("read `t.`");
                self.facts.temp_struct |= path.len() > 1;
                let deps = self.temps.get(&joined).cloned().unwrap_or_default();
                self.facts.reads.extend(deps.iter().cloned());
                deps
            }
            VariableSpace::Context => {
                self.feature("read `c.`");
                self.read(Read::Context(joined))
            }
        }
    }
}

pub fn is_random(name: &str) -> bool {
    matches!(
        name,
        "random" | "random_integer" | "die_roll" | "die_roll_integer"
    )
}

fn binary_feature(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "`+`",
        BinaryOp::Subtract => "`-`",
        BinaryOp::Multiply => "`*`",
        BinaryOp::Divide => "`/`",
        BinaryOp::Less => "`<`",
        BinaryOp::LessEqual => "`<=`",
        BinaryOp::Greater => "`>`",
        BinaryOp::GreaterEqual => "`>=`",
        BinaryOp::Equal => "`==`",
        BinaryOp::NotEqual => "`!=`",
        BinaryOp::And => "`&&`",
        BinaryOp::Or => "`||`",
    }
}
