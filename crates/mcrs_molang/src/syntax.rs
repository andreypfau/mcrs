use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::diagnostic::Span;
use crate::value::ResourceKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Syntax {
    pub body: Body,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Simple(Expr),
    Complex(Vec<Stmt>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariableSpace {
    Variable,
    Temp,
    Context,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Number(f32),
    Bool(bool),
    String(Box<str>),
    This,
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Conditional {
        condition: Box<Expr>,
        then: Box<Expr>,
        otherwise: Option<Box<Expr>>,
    },
    Coalesce(Box<Expr>, Box<Expr>),
    Block(Vec<Stmt>),
    AssignGroup(Box<Expr>, Box<Expr>),
    Query {
        name: Box<str>,
        arguments: Option<Vec<Expr>>,
        path: Vec<Box<str>>,
    },
    Math {
        name: Box<str>,
        arguments: Option<Vec<Expr>>,
    },
    Variable {
        space: VariableSpace,
        path: Vec<Box<str>>,
    },
    Resource {
        kind: ResourceKind,
        name: Box<str>,
    },
    Array(Box<str>),
    Index(Box<Expr>, Box<Expr>),
    Arrow(Box<Expr>, Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Empty,
    Expr(Expr),
    Assign(Expr, Expr),
    Return(Expr),
    Block(Vec<Stmt>),
    Break,
    Continue,
    Loop(Expr, Vec<Stmt>),
    ForEach(Expr, Expr, Vec<Stmt>),
}
