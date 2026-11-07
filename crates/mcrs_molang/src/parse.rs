use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use crate::diagnostic::{Diagnostic, ErrorKind, Span};
use crate::lexer::{self, Punct, Token, TokenKind};
use crate::syntax::{
    BinaryOp, Body, Expr, ExprKind, Stmt, StmtKind, Syntax, UnaryOp, VariableSpace,
};
use crate::value::ResourceKind;

/// Parses an expression. The parser records each syntax error and continues at the next `;`
/// or closing brace, so one call reports every error of the text.
pub fn parse(source: &str) -> Result<Syntax, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    if !source.is_ascii() {
        report_non_ascii(source, &mut diagnostics);
    }
    let tokens = lexer::lex(source, &mut diagnostics);
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        diagnostics,
    };
    let body = parser.body();
    if parser.diagnostics.is_empty() {
        Ok(Syntax { body })
    } else {
        Err(parser.diagnostics)
    }
}

fn report_non_ascii(source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut in_string = false;
    for (at, character) in source.char_indices() {
        if character == '\'' {
            in_string = !in_string;
        } else if !in_string && !character.is_ascii() {
            diagnostics.push(Diagnostic::new(
                ErrorKind::Syntax,
                Span::new(at..at + character.len_utf8()),
                "a character outside a string literal that is not ASCII",
            ));
            return;
        }
    }
}

/// The parse of the current statement stopped at an error that is already recorded.
struct Failed;

type Parsed<T> = Result<T, Failed>;

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    diagnostics: Vec<Diagnostic>,
}

const END: Token = Token {
    kind: TokenKind::End,
    span: Span { start: 0, end: 0 },
};

impl Parser<'_> {
    fn peek(&self) -> &Token {
        self.tokens.get(self.at).unwrap_or(&END)
    }

    fn peek_at(&self, offset: usize) -> &Token {
        self.tokens.get(self.at + offset).unwrap_or(&END)
    }

    fn is_punct(&self, punct: Punct) -> bool {
        self.peek().kind == TokenKind::Punct(punct)
    }

    fn eat(&mut self, punct: Punct) -> bool {
        let found = self.is_punct(punct);
        if found {
            self.at += 1;
        }
        found
    }

    fn previous_span(&self) -> Span {
        self.at
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or_else(Span::default, |token| token.span)
    }

    fn fail<T>(
        &mut self,
        kind: ErrorKind,
        span: Span,
        message: impl Into<alloc::string::String>,
    ) -> Parsed<T> {
        self.diagnostics.push(Diagnostic::new(kind, span, message));
        Err(Failed)
    }

    fn unexpected<T>(&mut self, wanted: &str) -> Parsed<T> {
        let token = self.peek().clone();
        let found = match &token.kind {
            TokenKind::Number(_) => "a number".into(),
            TokenKind::String(_) => "a string".into(),
            TokenKind::Name(name) => format!("`{name}`"),
            TokenKind::Punct(punct) => format!("`{}`", punct.text()),
            TokenKind::End => "the end of the expression".into(),
        };
        self.fail(
            ErrorKind::Syntax,
            token.span,
            format!("{found} where {wanted} is expected"),
        )
    }

    fn expect(&mut self, punct: Punct) -> Parsed<()> {
        if self.eat(punct) {
            Ok(())
        } else {
            self.unexpected(&format!("`{}`", punct.text()))
        }
    }

    fn body(&mut self) -> Body {
        let list = self.statement_list(Terminator::End);
        if !list.saw_semicolon
            && let [single] = list.statements.as_slice()
            && let StmtKind::Expr(expr) = &single.kind
        {
            return Body::Simple(expr.clone());
        }
        if list.statements.is_empty() && !list.saw_semicolon {
            return Body::Simple(Expr {
                kind: ExprKind::Number(0.0),
                span: Span::default(),
            });
        }
        Body::Complex(list.statements)
    }

    fn statement_list(&mut self, terminator: Terminator) -> StatementList {
        let mut list = StatementList {
            statements: Vec::new(),
            saw_semicolon: false,
        };
        loop {
            if self.at_terminator(terminator) {
                return list;
            }
            if self.eat(Punct::Semicolon) {
                list.saw_semicolon = true;
                list.statements.push(Stmt {
                    kind: StmtKind::Empty,
                    span: self.previous_span(),
                });
                continue;
            }
            match self.statement() {
                Ok(statement) => list.statements.push(statement),
                Err(Failed) => self.recover(terminator),
            }
            if self.eat(Punct::Semicolon) {
                list.saw_semicolon = true;
            } else if !self.at_terminator(terminator) {
                let wanted = match terminator {
                    Terminator::End => "`;`",
                    Terminator::Brace => "`;` or `}`",
                };
                let _: Parsed<()> = self.unexpected(wanted);
                self.recover(terminator);
                self.eat(Punct::Semicolon);
            }
        }
    }

    fn at_terminator(&self, terminator: Terminator) -> bool {
        match terminator {
            Terminator::End => self.peek().kind == TokenKind::End,
            Terminator::Brace => {
                self.is_punct(Punct::CloseBrace) || self.peek().kind == TokenKind::End
            }
        }
    }

    /// Skips to the next `;` or terminator of the current nesting level.
    fn recover(&mut self, terminator: Terminator) {
        let mut depth = 0usize;
        loop {
            match &self.peek().kind {
                TokenKind::End => return,
                TokenKind::Punct(Punct::OpenBrace) => depth += 1,
                TokenKind::Punct(Punct::CloseBrace) => {
                    if depth == 0 {
                        if terminator == Terminator::Brace {
                            return;
                        }
                    } else {
                        depth -= 1;
                    }
                }
                TokenKind::Punct(Punct::Semicolon) if depth == 0 => return,
                _ => {}
            }
            self.at += 1;
        }
    }

    fn keyword(&self) -> Option<&str> {
        match &self.peek().kind {
            TokenKind::Name(name) if self.peek_at(1).kind != TokenKind::Punct(Punct::Dot) => {
                Some(name)
            }
            _ => None,
        }
    }

    fn statement(&mut self) -> Parsed<Stmt> {
        let start = self.peek().span;
        let kind = match self.keyword() {
            Some("return") => {
                self.at += 1;
                StmtKind::Return(self.operand()?)
            }
            Some("break") => {
                self.at += 1;
                StmtKind::Break
            }
            Some("continue") => {
                self.at += 1;
                StmtKind::Continue
            }
            Some("loop") => {
                self.at += 1;
                self.expect(Punct::OpenParen)?;
                let count = self.operand()?;
                self.expect(Punct::Comma)?;
                let body = self.block()?;
                self.expect(Punct::CloseParen)?;
                StmtKind::Loop(count, body)
            }
            Some("for_each") => {
                self.at += 1;
                self.expect(Punct::OpenParen)?;
                let variable = self.operand()?;
                if !matches!(
                    variable.kind,
                    ExprKind::Variable {
                        space: VariableSpace::Variable | VariableSpace::Temp,
                        ..
                    }
                ) {
                    return self.fail(
                        ErrorKind::Syntax,
                        variable.span,
                        "`for_each` needs a variable",
                    );
                }
                self.expect(Punct::Comma)?;
                let array = self.operand()?;
                self.expect(Punct::Comma)?;
                let body = self.block()?;
                self.expect(Punct::CloseParen)?;
                StmtKind::ForEach(variable, array, body)
            }
            _ if self.is_punct(Punct::OpenBrace) => StmtKind::Block(self.block()?),
            _ => {
                let operand = self.operand()?;
                if self.eat(Punct::Equal) {
                    self.check_lvalue(&operand)?;
                    let value = self.operand()?;
                    StmtKind::Assign(operand, value)
                } else {
                    StmtKind::Expr(operand)
                }
            }
        };
        Ok(Stmt {
            kind,
            span: start.to(self.previous_span()),
        })
    }

    fn check_lvalue(&mut self, target: &Expr) -> Parsed<()> {
        match &target.kind {
            ExprKind::Variable {
                space: VariableSpace::Variable | VariableSpace::Temp,
                ..
            } => Ok(()),
            ExprKind::Arrow(_, member)
                if matches!(
                    member.kind,
                    ExprKind::Variable {
                        space: VariableSpace::Variable | VariableSpace::Temp,
                        ..
                    }
                ) =>
            {
                Ok(())
            }
            ExprKind::Variable {
                space: VariableSpace::Context,
                ..
            }
            | ExprKind::Query { .. }
            | ExprKind::Math { .. }
            | ExprKind::Resource { .. }
            | ExprKind::Array(_)
            | ExprKind::This
            | ExprKind::Arrow(..) => self.fail(
                ErrorKind::ReadOnly,
                target.span,
                "an assignment to a value that cannot be written",
            ),
            _ => self.fail(
                ErrorKind::Syntax,
                target.span,
                "the left side of `=` is not a variable",
            ),
        }
    }

    fn block(&mut self) -> Parsed<Vec<Stmt>> {
        self.expect(Punct::OpenBrace)?;
        let list = self.statement_list(Terminator::Brace);
        self.expect(Punct::CloseBrace)?;
        Ok(list.statements)
    }

    fn block_expr(&mut self) -> Parsed<Expr> {
        let start = self.peek().span;
        let statements = self.block()?;
        Ok(Expr {
            kind: ExprKind::Block(statements),
            span: start.to(self.previous_span()),
        })
    }

    fn operand(&mut self) -> Parsed<Expr> {
        let left = self.conditional()?;
        if !self.eat(Punct::QuestionQuestion) {
            return Ok(left);
        }
        let right = if self.is_punct(Punct::OpenBrace) {
            self.block_expr()?
        } else {
            self.operand()?
        };
        let span = left.span.to(right.span);
        Ok(Expr {
            kind: ExprKind::Coalesce(Box::new(left), Box::new(right)),
            span,
        })
    }

    fn conditional(&mut self) -> Parsed<Expr> {
        let condition = self.binary(0)?;
        if !self.eat(Punct::Question) {
            return Ok(condition);
        }
        let then = self.branch()?;
        let otherwise = if self.eat(Punct::Colon) {
            Some(Box::new(self.branch()?))
        } else {
            None
        };
        let end = otherwise.as_ref().map_or(then.span, |expr| expr.span);
        let span = condition.span.to(end);
        Ok(Expr {
            kind: ExprKind::Conditional {
                condition: Box::new(condition),
                then: Box::new(then),
                otherwise,
            },
            span,
        })
    }

    fn branch(&mut self) -> Parsed<Expr> {
        if self.is_punct(Punct::OpenBrace) {
            return self.block_expr();
        }
        if self.is_punct(Punct::OpenParen) {
            let restart = self.at;
            let diagnostics = self.diagnostics.len();
            self.at += 1;
            if let Ok(target) = self.binary(LEVELS.len())
                && self.eat(Punct::Equal)
            {
                self.check_lvalue(&target)?;
                let value = self.operand()?;
                self.expect(Punct::CloseParen)?;
                let span = target.span.to(self.previous_span());
                return Ok(Expr {
                    kind: ExprKind::AssignGroup(Box::new(target), Box::new(value)),
                    span,
                });
            }
            self.at = restart;
            self.diagnostics.truncate(diagnostics);
        }
        self.conditional()
    }

    fn binary(&mut self, level: usize) -> Parsed<Expr> {
        let Some(operators) = LEVELS.get(level) else {
            return self.unary();
        };
        let mut left = self.binary(level + 1)?;
        loop {
            let TokenKind::Punct(punct) = self.peek().kind else {
                return Ok(left);
            };
            let Some((_, op)) = operators.iter().find(|(candidate, _)| *candidate == punct) else {
                return Ok(left);
            };
            self.at += 1;
            let right = self.binary(level + 1)?;
            let span = left.span.to(right.span);
            left = Expr {
                kind: ExprKind::Binary(*op, Box::new(left), Box::new(right)),
                span,
            };
        }
    }

    fn unary(&mut self) -> Parsed<Expr> {
        let start = self.peek().span;
        let op = if self.eat(Punct::Bang) {
            UnaryOp::Not
        } else if self.eat(Punct::Minus) {
            UnaryOp::Negate
        } else {
            return self.postfix();
        };
        let operand = self.unary()?;
        let span = start.to(operand.span);
        Ok(Expr {
            kind: ExprKind::Unary(op, Box::new(operand)),
            span,
        })
    }

    fn postfix(&mut self) -> Parsed<Expr> {
        let mut expr = self.primary()?;
        loop {
            if self.eat(Punct::OpenBracket) {
                let index = self.operand()?;
                self.expect(Punct::CloseBracket)?;
                let span = expr.span.to(self.previous_span());
                expr = Expr {
                    kind: ExprKind::Index(Box::new(expr), Box::new(index)),
                    span,
                };
            } else if self.eat(Punct::Arrow) {
                let member = self.reference()?;
                let span = expr.span.to(member.span);
                expr = Expr {
                    kind: ExprKind::Arrow(Box::new(expr), Box::new(member)),
                    span,
                };
            } else {
                return Ok(expr);
            }
        }
    }

    fn primary(&mut self) -> Parsed<Expr> {
        let token = self.peek().clone();
        let kind = match &token.kind {
            TokenKind::Number(value) => ExprKind::Number(*value),
            TokenKind::String(text) => ExprKind::String(text.clone()),
            TokenKind::Punct(Punct::OpenParen) => {
                self.at += 1;
                let inner = self.operand()?;
                self.expect(Punct::CloseParen)?;
                return Ok(Expr {
                    kind: inner.kind,
                    span: token.span.to(self.previous_span()),
                });
            }
            TokenKind::Name(_) if self.peek_at(1).kind == TokenKind::Punct(Punct::Dot) => {
                return self.reference();
            }
            TokenKind::Name(name) => match &**name {
                "true" => ExprKind::Bool(true),
                "false" => ExprKind::Bool(false),
                "this" => ExprKind::This,
                "return" | "loop" | "for_each" | "break" | "continue" => {
                    return self.unexpected("an operand");
                }
                "query" | "q" | "variable" | "v" | "temp" | "t" | "context" | "c" | "math"
                | "geometry" | "material" | "texture" | "array" => {
                    return self.fail(
                        ErrorKind::Syntax,
                        token.span,
                        format!("the namespace `{name}` needs `.` and a name"),
                    );
                }
                _ => {
                    return self.fail(
                        ErrorKind::Reserved,
                        token.span,
                        format!("`{name}` is not a keyword and has no namespace"),
                    );
                }
            },
            _ => return self.unexpected("an operand"),
        };
        self.at += 1;
        Ok(Expr {
            kind,
            span: token.span,
        })
    }

    fn name(&mut self) -> Parsed<Box<str>> {
        if let TokenKind::Name(name) = &self.peek().kind {
            let name = name.clone();
            self.at += 1;
            Ok(name)
        } else {
            self.unexpected("a name")
        }
    }

    fn path(&mut self) -> Parsed<Vec<Box<str>>> {
        let mut path = Vec::new();
        path.push(self.name()?);
        while self.is_punct(Punct::Dot) && matches!(self.peek_at(1).kind, TokenKind::Name(_)) {
            self.at += 1;
            path.push(self.name()?);
        }
        Ok(path)
    }

    fn reference(&mut self) -> Parsed<Expr> {
        let start = self.peek().span;
        let TokenKind::Name(space) = self.peek().kind.clone() else {
            return self.unexpected("a reference");
        };
        if self.peek_at(1).kind != TokenKind::Punct(Punct::Dot) {
            return self.unexpected("a reference");
        }
        self.at += 2;
        let kind = match &*space {
            "query" | "q" => {
                let name = self.name()?;
                let arguments = self.arguments()?;
                let mut path = Vec::new();
                if self.eat(Punct::Dot) {
                    path = self.path()?;
                }
                ExprKind::Query {
                    name,
                    arguments,
                    path,
                }
            }
            "math" => {
                let name = self.name()?;
                let arguments = self.arguments()?;
                ExprKind::Math { name, arguments }
            }
            "variable" | "v" => ExprKind::Variable {
                space: VariableSpace::Variable,
                path: self.path()?,
            },
            "temp" | "t" => ExprKind::Variable {
                space: VariableSpace::Temp,
                path: self.path()?,
            },
            "context" | "c" => ExprKind::Variable {
                space: VariableSpace::Context,
                path: self.path()?,
            },
            "geometry" => resource(ResourceKind::Geometry, self.name()?),
            "material" => resource(ResourceKind::Material, self.name()?),
            "texture" => resource(ResourceKind::Texture, self.name()?),
            "array" => ExprKind::Array(self.name()?),
            _ => {
                return self.fail(
                    ErrorKind::Reserved,
                    start,
                    format!("`{space}` is not a namespace"),
                );
            }
        };
        Ok(Expr {
            kind,
            span: start.to(self.previous_span()),
        })
    }

    fn arguments(&mut self) -> Parsed<Option<Vec<Expr>>> {
        if !self.eat(Punct::OpenParen) {
            return Ok(None);
        }
        let mut arguments = Vec::new();
        if self.eat(Punct::CloseParen) {
            return Ok(Some(arguments));
        }
        loop {
            arguments.push(self.operand()?);
            if self.eat(Punct::CloseParen) {
                return Ok(Some(arguments));
            }
            self.expect(Punct::Comma)?;
        }
    }
}

fn resource(kind: ResourceKind, name: Box<str>) -> ExprKind {
    ExprKind::Resource { kind, name }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Terminator {
    End,
    Brace,
}

struct StatementList {
    statements: Vec<Stmt>,
    saw_semicolon: bool,
}

/// The binary levels from the lowest precedence to the highest.
const LEVELS: [&[(Punct, BinaryOp)]; 6] = [
    &[(Punct::OrOr, BinaryOp::Or)],
    &[(Punct::AndAnd, BinaryOp::And)],
    &[
        (Punct::EqualEqual, BinaryOp::Equal),
        (Punct::BangEqual, BinaryOp::NotEqual),
    ],
    &[
        (Punct::Less, BinaryOp::Less),
        (Punct::LessEqual, BinaryOp::LessEqual),
        (Punct::Greater, BinaryOp::Greater),
        (Punct::GreaterEqual, BinaryOp::GreaterEqual),
    ],
    &[
        (Punct::Plus, BinaryOp::Add),
        (Punct::Minus, BinaryOp::Subtract),
    ],
    &[
        (Punct::Star, BinaryOp::Multiply),
        (Punct::Slash, BinaryOp::Divide),
    ],
];
