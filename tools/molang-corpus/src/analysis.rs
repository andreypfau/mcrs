use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Branch, CallSpace, Expr, PathSpace, Program, ResourceSpace, Stmt};
use crate::lexer::Punct;

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
pub fn analyse(program: &Program) -> Facts {
    let mut walker = Walker {
        facts: Facts::default(),
        temps: BTreeMap::new(),
        arrow_depth: 0,
    };
    walker.statements(&program.statements, &BTreeSet::new());
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
        self.facts.nodes += 1;
        self.facts.statements += 1;
        match statement {
            Stmt::Return(value) => {
                self.feature("return");
                self.expr(value, control);
            }
            Stmt::Break => self.feature("break"),
            Stmt::Continue => self.feature("continue"),
            Stmt::Assign { target, value } => self.assign(target, value, control),
            Stmt::Loop { count, body } => {
                self.feature("loop");
                let mut inner = control.clone();
                inner.extend(self.expr(count, control));
                self.statements(body, &inner);
            }
            Stmt::ForEach {
                variable,
                collection,
                body,
            } => {
                self.feature("for_each");
                let mut inner = control.clone();
                inner.extend(self.expr(collection, control));
                self.write_target(variable, inner.clone());
                self.statements(body, &inner);
            }
            Stmt::Block(body) => {
                self.feature("block statement");
                self.statements(body, control);
            }
            Stmt::Expr(expr) => {
                self.expr(expr, control);
            }
        }
    }

    fn assign(&mut self, target: &Expr, value: &Expr, control: &BTreeSet<Read>) {
        let mut deps = self.expr(value, control);
        deps.extend(control.iter().cloned());
        match target {
            Expr::Path { space, names } => {
                let name = match space {
                    PathSpace::Variable => {
                        if names.len() > 1 {
                            self.feature("struct write `v.a.b = `");
                        }
                        "assign `v.`"
                    }
                    PathSpace::Temp => "assign `t.`",
                    PathSpace::Context => "assign `c.` (read-only)",
                };
                self.feature(name);
            }
            Expr::Arrow { target, .. } => {
                self.feature("assign through `->`");
                deps.extend(self.expr(target, control));
            }
            _ => {}
        }
        self.write_target(target, deps);
    }

    fn write_target(&mut self, target: &Expr, deps: BTreeSet<Read>) {
        let Expr::Path { space, names } = target else {
            return;
        };
        self.facts.nodes += 1;
        let joined = names.join(".");
        match space {
            PathSpace::Variable => {
                self.facts.variable_names.insert(joined.clone());
                self.facts.writes.push(VariableWrite { name: joined, deps });
            }
            PathSpace::Temp => {
                self.facts.temp_struct |= names.len() > 1;
                self.temps.entry(joined).or_default().extend(deps);
            }
            PathSpace::Context => {}
        }
    }

    fn branch(&mut self, branch: &Branch, control: &BTreeSet<Read>) -> BTreeSet<Read> {
        match branch {
            Branch::Expr(expr) => self.expr(expr, control),
            Branch::Block(body) => {
                self.facts.nodes += 1;
                self.feature("block branch `? {}`");
                self.statements(body, control);
                BTreeSet::new()
            }
        }
    }

    fn expr(&mut self, expr: &Expr, control: &BTreeSet<Read>) -> BTreeSet<Read> {
        self.facts.nodes += 1;
        match expr {
            Expr::Number(_) => BTreeSet::new(),
            Expr::Str(_) => {
                self.feature("string literal");
                BTreeSet::new()
            }
            Expr::This => {
                self.feature("`this`");
                self.read(Read::This)
            }
            Expr::Call { space, name, args } => self.call(*space, name, args.as_deref(), control),
            Expr::Path { space, names } => self.path(*space, names),
            Expr::Resource { space, .. } => {
                self.feature(match space {
                    ResourceSpace::Geometry => "`geometry.` reference",
                    ResourceSpace::Material => "`material.` reference",
                    ResourceSpace::Texture => "`texture.` reference",
                    ResourceSpace::Array => "`array.` reference",
                });
                self.read(Read::Resource)
            }
            Expr::Unary { op, operand } => {
                self.feature(if *op == Punct::Bang {
                    "unary `!`"
                } else {
                    "unary `-`"
                });
                self.expr(operand, control)
            }
            Expr::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs, control),
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                self.feature(if else_branch.is_some() {
                    "`? :`"
                } else {
                    "binary `?`"
                });
                let mut reads = self.expr(condition, control);
                let mut inner = control.clone();
                inner.extend(reads.iter().cloned());
                reads.extend(self.branch(then_branch, &inner));
                if let Some(else_branch) = else_branch {
                    reads.extend(self.branch(else_branch, &inner));
                }
                reads
            }
            Expr::Index { target, index } => {
                self.feature("index `[]`");
                let mut reads = self.expr(target, control);
                reads.extend(self.expr(index, control));
                reads
            }
            Expr::Arrow { target, reference } => {
                self.feature("`->`");
                let mut reads = self.expr(target, control);
                self.arrow_depth += 1;
                reads.extend(self.expr(reference, control));
                self.arrow_depth -= 1;
                reads
            }
        }
    }

    fn binary(
        &mut self,
        op: Punct,
        lhs: &Expr,
        rhs: &Expr,
        control: &BTreeSet<Read>,
    ) -> BTreeSet<Read> {
        self.feature(binary_feature(op));
        let is_string = |expr: &Expr| matches!(expr, Expr::Str(_));
        if is_string(lhs) || is_string(rhs) {
            self.feature(match op {
                Punct::EqEq | Punct::NotEq => "string compare `==`/`!=`",
                _ => "string operand of another operator",
            });
        }
        let mut reads = self.expr(lhs, control);
        reads.extend(self.expr(rhs, control));
        reads
    }

    fn call(
        &mut self,
        space: CallSpace,
        name: &str,
        args: Option<&[Expr]>,
        control: &BTreeSet<Read>,
    ) -> BTreeSet<Read> {
        let mut reads = BTreeSet::new();
        for arg in args.unwrap_or_default() {
            reads.extend(self.expr(arg, control));
        }
        let arity = args.map(<[Expr]>::len);
        match space {
            CallSpace::Query => {
                self.feature("query");
                let literal_args = args
                    .unwrap_or_default()
                    .iter()
                    .all(|arg| matches!(arg, Expr::Number(_) | Expr::Str(_)));
                self.facts.queries.push(QueryUse {
                    name: name.to_owned(),
                    arity,
                    literal_args,
                    via_arrow: self.arrow_depth > 0,
                });
                reads.extend(self.read(Read::Query(name.to_owned())));
            }
            CallSpace::Math => {
                self.feature("math");
                self.facts.math.push(MathUse {
                    name: name.to_owned(),
                    arity,
                });
                if is_random(name) {
                    reads.extend(self.read(Read::Random));
                }
            }
        }
        reads
    }

    fn path(&mut self, space: PathSpace, names: &[String]) -> BTreeSet<Read> {
        let joined = names.join(".");
        match space {
            PathSpace::Variable => {
                self.feature(if names.len() > 1 {
                    "read `v.a.b` (struct path)"
                } else {
                    "read `v.`"
                });
                self.facts.variable_names.insert(joined.clone());
                self.read(Read::Variable(joined))
            }
            PathSpace::Temp => {
                self.feature("read `t.`");
                self.facts.temp_struct |= names.len() > 1;
                let deps = self.temps.get(&joined).cloned().unwrap_or_default();
                self.facts.reads.extend(deps.iter().cloned());
                deps
            }
            PathSpace::Context => {
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

fn binary_feature(op: Punct) -> &'static str {
    use Punct::*;
    match op {
        Plus => "`+`",
        Minus => "`-`",
        Star => "`*`",
        Slash => "`/`",
        Less => "`<`",
        LessEq => "`<=`",
        Greater => "`>`",
        GreaterEq => "`>=`",
        EqEq => "`==`",
        NotEq => "`!=`",
        AndAnd => "`&&`",
        OrOr => "`||`",
        Coalesce => "`??`",
        _ => "other operator",
    }
}
