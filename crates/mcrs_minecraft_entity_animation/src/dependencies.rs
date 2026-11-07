use mcrs_molang::{Body, Expr, ExprKind, Stmt, StmtKind, Syntax, VariableSpace};

/// The names that an expression reads and writes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Dependencies {
    pub queries: Vec<Box<str>>,
    pub reads: Vec<Box<str>>,
    pub writes: Vec<Box<str>>,
    /// The expression draws a random number or reads a context value: its result can differ
    /// with the same queries and variables.
    pub volatile: bool,
}

impl Dependencies {
    pub fn of(syntax: &Syntax) -> Self {
        let mut dependencies = Self::default();
        match &syntax.body {
            Body::Simple(expr) => dependencies.expr(expr),
            Body::Complex(statements) => dependencies.statements(statements),
        }
        dependencies
    }

    /// Whether the expression reads a variable that it also writes, so that its result depends
    /// on the number of runs.
    pub fn reads_own_write(&self) -> bool {
        self.writes.iter().any(|name| self.reads.contains(name))
    }

    fn statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Empty | StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(expr) | StmtKind::Return(expr) => self.expr(expr),
            StmtKind::Assign(target, value) => {
                self.target(target);
                self.expr(value);
            }
            StmtKind::Block(body) => self.statements(body),
            StmtKind::Loop(count, body) => {
                self.expr(count);
                self.statements(body);
            }
            StmtKind::ForEach(variable, collection, body) => {
                self.target(variable);
                self.expr(collection);
                self.statements(body);
            }
        }
    }

    fn target(&mut self, target: &Expr) {
        match &target.kind {
            ExprKind::Variable {
                space: VariableSpace::Variable,
                path,
            } => push(&mut self.writes, path),
            _ => self.expr(target),
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Number(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::This
            | ExprKind::Resource { .. }
            | ExprKind::Array(_) => {}
            ExprKind::Unary(_, operand) => self.expr(operand),
            ExprKind::Binary(_, left, right)
            | ExprKind::Coalesce(left, right)
            | ExprKind::Index(left, right)
            | ExprKind::Arrow(left, right) => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Conditional {
                condition,
                then,
                otherwise,
            } => {
                self.expr(condition);
                self.expr(then);
                if let Some(otherwise) = otherwise {
                    self.expr(otherwise);
                }
            }
            ExprKind::Block(statements) => self.statements(statements),
            ExprKind::AssignGroup(target, value) => {
                self.target(target);
                self.expr(value);
            }
            ExprKind::Query {
                name, arguments, ..
            } => {
                if !self.queries.contains(name) {
                    self.queries.push(name.clone());
                }
                self.arguments(arguments.as_deref());
            }
            ExprKind::Math { name, arguments } => {
                if name.starts_with("random") || name.starts_with("die_roll") {
                    self.volatile = true;
                }
                self.arguments(arguments.as_deref());
            }
            ExprKind::Variable { space, path } => match space {
                VariableSpace::Variable => push(&mut self.reads, path),
                VariableSpace::Context => self.volatile = true,
                VariableSpace::Temp => {}
            },
        }
    }

    fn arguments(&mut self, arguments: Option<&[Expr]>) {
        for argument in arguments.unwrap_or_default() {
            self.expr(argument);
        }
    }
}

fn push(names: &mut Vec<Box<str>>, path: &[Box<str>]) {
    if let Some(name) = path.first()
        && !names.contains(name)
    {
        names.push(name.clone());
    }
}
