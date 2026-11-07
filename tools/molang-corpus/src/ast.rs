use crate::lexer::Punct;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallSpace {
    Query,
    Math,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathSpace {
    Variable,
    Temp,
    Context,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceSpace {
    Geometry,
    Material,
    Texture,
    Array,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Number(f64),
    Str(String),
    This,
    Call {
        space: CallSpace,
        name: String,
        args: Option<Vec<Expr>>,
    },
    Path {
        space: PathSpace,
        names: Vec<String>,
    },
    Resource {
        space: ResourceSpace,
        name: String,
    },
    Unary {
        op: Punct,
        operand: Box<Expr>,
    },
    Binary {
        op: Punct,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Conditional {
        condition: Box<Expr>,
        then_branch: Box<Branch>,
        else_branch: Option<Box<Branch>>,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    Arrow {
        target: Box<Expr>,
        reference: Box<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Branch {
    Expr(Expr),
    Block(Vec<Stmt>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Return(Expr),
    Break,
    Continue,
    Assign {
        target: Expr,
        value: Expr,
    },
    Loop {
        count: Expr,
        body: Vec<Stmt>,
    },
    ForEach {
        variable: Expr,
        collection: Expr,
        body: Vec<Stmt>,
    },
    Block(Vec<Stmt>),
    Expr(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form {
    Simple,
    Complex,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub form: Form,
    pub statements: Vec<Stmt>,
}
