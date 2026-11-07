use std::fmt;

use crate::ast::{Branch, CallSpace, Expr, Form, PathSpace, Program, ResourceSpace, Stmt};
use crate::lexer::{self, LexErrorKind, Punct, Token, TokenKind};

/// A mismatch with the grammar of MOL-SYN-001.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseErrorKind {
    Lex(LexErrorKind),
    Unexpected {
        found: String,
        expected: &'static str,
    },
    End {
        expected: &'static str,
    },
    /// A name with no namespace that is not a keyword.
    BareName(String),
    AssignTarget,
    TrailingComma,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::Lex(LexErrorKind::NonAscii) => {
                formatter.write_str("non-ASCII character")
            }
            ParseErrorKind::Lex(LexErrorKind::UnknownChar(character)) => {
                write!(formatter, "unknown character `{character}`")
            }
            ParseErrorKind::Lex(LexErrorKind::UnterminatedString) => {
                formatter.write_str("unterminated string")
            }
            ParseErrorKind::Unexpected { found, expected } => {
                write!(formatter, "`{found}` where {expected} is expected")
            }
            ParseErrorKind::End { expected } => {
                write!(formatter, "end where {expected} is expected")
            }
            ParseErrorKind::BareName(name) => write!(formatter, "name `{name}` without namespace"),
            ParseErrorKind::AssignTarget => formatter.write_str("assignment to a non-variable"),
            ParseErrorKind::TrailingComma => formatter.write_str("trailing comma in arguments"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub program: Program,
    pub tokens: Vec<Token>,
}

pub fn parse(source: &str) -> Result<Parsed, ParseError> {
    let tokens = lexer::tokenize(source).map_err(|error| ParseError {
        kind: ParseErrorKind::Lex(error.kind),
        offset: error.offset,
    })?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        end: source.len(),
    };
    let list = parser.statements(Terminator::End)?;
    let single_operand = matches!(list.statements.as_slice(), [Stmt::Expr(_)]);
    let form = if list.semicolons || !(single_operand || list.statements.is_empty()) {
        Form::Complex
    } else {
        Form::Simple
    };
    let program = Program {
        form,
        statements: list.statements,
    };
    Ok(Parsed { program, tokens })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Terminator {
    End,
    Brace,
}

struct StatementList {
    statements: Vec<Stmt>,
    semicolons: bool,
}

struct Parser<'tokens> {
    tokens: &'tokens [Token],
    pos: usize,
    end: usize,
}

type ParseResult<T> = Result<T, ParseError>;

impl Parser<'_> {
    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|token| &token.kind)
    }

    fn peek_punct(&self) -> Option<Punct> {
        match self.peek() {
            Some(TokenKind::Punct(punct)) => Some(*punct),
            _ => None,
        }
    }

    fn peek_name(&self) -> Option<&str> {
        match self.peek() {
            Some(TokenKind::Name(name)) => Some(name),
            _ => None,
        }
    }

    fn eat(&mut self, punct: Punct) -> bool {
        let found = self.peek_punct() == Some(punct);
        if found {
            self.pos += 1;
        }
        found
    }

    fn error(&self, expected: &'static str) -> ParseError {
        match self.tokens.get(self.pos) {
            Some(token) => ParseError {
                kind: ParseErrorKind::Unexpected {
                    found: token.kind.to_string(),
                    expected,
                },
                offset: token.offset,
            },
            None => ParseError {
                kind: ParseErrorKind::End { expected },
                offset: self.end,
            },
        }
    }

    fn offset(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map_or(self.end, |token| token.offset)
    }

    fn expect(&mut self, punct: Punct, expected: &'static str) -> ParseResult<()> {
        if self.eat(punct) {
            Ok(())
        } else {
            Err(self.error(expected))
        }
    }

    fn at_terminator(&self, terminator: Terminator) -> bool {
        match terminator {
            Terminator::End => self.peek().is_none(),
            Terminator::Brace => self.peek_punct() == Some(Punct::RBrace),
        }
    }

    fn statements(&mut self, terminator: Terminator) -> ParseResult<StatementList> {
        let mut statements = Vec::new();
        let mut semicolons = false;
        loop {
            while self.eat(Punct::Semi) {
                semicolons = true;
            }
            if self.at_terminator(terminator) {
                break;
            }
            statements.push(self.statement()?);
            if self.peek_punct() == Some(Punct::Semi) {
                continue;
            }
            if self.at_terminator(terminator) {
                break;
            }
            return Err(self.error("`;`"));
        }
        Ok(StatementList {
            statements,
            semicolons,
        })
    }

    fn block(&mut self) -> ParseResult<Vec<Stmt>> {
        self.expect(Punct::LBrace, "`{`")?;
        let list = self.statements(Terminator::Brace)?;
        self.expect(Punct::RBrace, "`}`")?;
        Ok(list.statements)
    }

    fn statement(&mut self) -> ParseResult<Stmt> {
        match self.peek_name() {
            Some("return") => {
                self.pos += 1;
                return Ok(Stmt::Return(self.operand()?));
            }
            Some("break") => {
                self.pos += 1;
                return Ok(Stmt::Break);
            }
            Some("continue") => {
                self.pos += 1;
                return Ok(Stmt::Continue);
            }
            Some("loop") => return self.loop_statement(),
            Some("for_each") => return self.for_each_statement(),
            _ => {}
        }
        if self.peek_punct() == Some(Punct::LBrace) {
            return Ok(Stmt::Block(self.block()?));
        }
        let start = self.offset();
        let operand = self.operand()?;
        if !self.eat(Punct::Assign) {
            return Ok(Stmt::Expr(operand));
        }
        if !is_lvalue(&operand) {
            return Err(ParseError {
                kind: ParseErrorKind::AssignTarget,
                offset: start,
            });
        }
        Ok(Stmt::Assign {
            target: operand,
            value: self.operand()?,
        })
    }

    fn loop_statement(&mut self) -> ParseResult<Stmt> {
        self.pos += 1;
        self.expect(Punct::LParen, "`(`")?;
        let count = self.operand()?;
        self.expect(Punct::Comma, "`,`")?;
        let body = self.block()?;
        self.expect(Punct::RParen, "`)`")?;
        Ok(Stmt::Loop { count, body })
    }

    fn for_each_statement(&mut self) -> ParseResult<Stmt> {
        self.pos += 1;
        self.expect(Punct::LParen, "`(`")?;
        let variable = self.primary()?;
        if !matches!(variable, Expr::Path { .. }) {
            return Err(self.error("a variable"));
        }
        self.expect(Punct::Comma, "`,`")?;
        let collection = self.operand()?;
        self.expect(Punct::Comma, "`,`")?;
        let body = self.block()?;
        self.expect(Punct::RParen, "`)`")?;
        Ok(Stmt::ForEach {
            variable,
            collection,
            body,
        })
    }

    fn operand(&mut self) -> ParseResult<Expr> {
        let lhs = self.conditional()?;
        if !self.eat(Punct::Coalesce) {
            return Ok(lhs);
        }
        let rhs = self.operand()?;
        Ok(binary(Punct::Coalesce, lhs, rhs))
    }

    fn conditional(&mut self) -> ParseResult<Expr> {
        let condition = self.binary_level(0)?;
        if !self.eat(Punct::Question) {
            return Ok(condition);
        }
        let then_branch = self.branch()?;
        let else_branch = if self.eat(Punct::Colon) {
            Some(Box::new(self.branch()?))
        } else {
            None
        };
        Ok(Expr::Conditional {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
        })
    }

    fn branch(&mut self) -> ParseResult<Branch> {
        if self.peek_punct() == Some(Punct::LBrace) {
            return Ok(Branch::Block(self.block()?));
        }
        Ok(Branch::Expr(self.conditional()?))
    }

    fn binary_level(&mut self, level: usize) -> ParseResult<Expr> {
        let Some(operators) = LEVELS.get(level) else {
            return self.unary();
        };
        let mut lhs = self.binary_level(level + 1)?;
        while let Some(op) = self.peek_punct().filter(|op| operators.contains(op)) {
            self.pos += 1;
            let rhs = self.binary_level(level + 1)?;
            lhs = binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> ParseResult<Expr> {
        if let Some(op @ (Punct::Bang | Punct::Minus)) = self.peek_punct() {
            self.pos += 1;
            let operand = self.unary()?;
            return Ok(Expr::Unary {
                op,
                operand: Box::new(operand),
            });
        }
        self.postfix()
    }

    fn postfix(&mut self) -> ParseResult<Expr> {
        let mut expr = self.primary()?;
        loop {
            if self.eat(Punct::LBracket) {
                let index = self.operand()?;
                self.expect(Punct::RBracket, "`]`")?;
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index),
                };
            } else if self.eat(Punct::Arrow) {
                let reference = self.reference()?;
                expr = Expr::Arrow {
                    target: Box::new(expr),
                    reference: Box::new(reference),
                };
            } else {
                return Ok(expr);
            }
        }
    }

    fn primary(&mut self) -> ParseResult<Expr> {
        match self.peek() {
            Some(TokenKind::Number(value)) => {
                let value = *value;
                self.pos += 1;
                Ok(Expr::Number(value))
            }
            Some(TokenKind::String(text)) => {
                let text = text.clone();
                self.pos += 1;
                Ok(Expr::Str(text))
            }
            Some(TokenKind::Punct(Punct::LParen)) => {
                self.pos += 1;
                let inner = self.operand()?;
                self.expect(Punct::RParen, "`)`")?;
                Ok(inner)
            }
            Some(TokenKind::Name(name)) => match name.as_str() {
                "true" | "false" => {
                    let value = if name == "true" { 1.0 } else { 0.0 };
                    self.pos += 1;
                    Ok(Expr::Number(value))
                }
                "this" => {
                    self.pos += 1;
                    Ok(Expr::This)
                }
                _ => self.reference(),
            },
            _ => Err(self.error("an operand")),
        }
    }

    fn reference(&mut self) -> ParseResult<Expr> {
        let offset = self.offset();
        let Some(namespace) = self.peek_name().map(str::to_owned) else {
            return Err(self.error("a reference"));
        };
        let Some(kind) = namespace_kind(&namespace) else {
            return Err(ParseError {
                kind: ParseErrorKind::BareName(namespace),
                offset,
            });
        };
        self.pos += 1;
        self.expect(Punct::Dot, "`.` after a namespace")?;
        let first = self.name()?;
        match kind {
            Namespace::Call(space) => {
                let args = if self.peek_punct() == Some(Punct::LParen) {
                    Some(self.arguments()?)
                } else {
                    None
                };
                Ok(Expr::Call {
                    space,
                    name: first,
                    args,
                })
            }
            Namespace::Path(space) => {
                let mut names = vec![first];
                while self.peek_punct() == Some(Punct::Dot)
                    && matches!(
                        self.tokens.get(self.pos + 1).map(|token| &token.kind),
                        Some(TokenKind::Name(_))
                    )
                {
                    self.pos += 1;
                    names.push(self.name()?);
                }
                Ok(Expr::Path { space, names })
            }
            Namespace::Resource(space) => Ok(Expr::Resource { space, name: first }),
        }
    }

    fn name(&mut self) -> ParseResult<String> {
        let Some(name) = self.peek_name().map(str::to_owned) else {
            return Err(self.error("a name"));
        };
        self.pos += 1;
        Ok(name)
    }

    fn arguments(&mut self) -> ParseResult<Vec<Expr>> {
        self.pos += 1;
        let mut args = Vec::new();
        if self.eat(Punct::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.operand()?);
            if self.eat(Punct::RParen) {
                return Ok(args);
            }
            self.expect(Punct::Comma, "`,` or `)`")?;
            if self.peek_punct() == Some(Punct::RParen) {
                return Err(ParseError {
                    kind: ParseErrorKind::TrailingComma,
                    offset: self.offset(),
                });
            }
        }
    }
}

const LEVELS: [&[Punct]; 6] = [
    &[Punct::OrOr],
    &[Punct::AndAnd],
    &[Punct::EqEq, Punct::NotEq],
    &[Punct::Less, Punct::LessEq, Punct::Greater, Punct::GreaterEq],
    &[Punct::Plus, Punct::Minus],
    &[Punct::Star, Punct::Slash],
];

enum Namespace {
    Call(CallSpace),
    Path(PathSpace),
    Resource(ResourceSpace),
}

fn namespace_kind(name: &str) -> Option<Namespace> {
    Some(match name {
        "query" | "q" => Namespace::Call(CallSpace::Query),
        "math" => Namespace::Call(CallSpace::Math),
        "variable" | "v" => Namespace::Path(PathSpace::Variable),
        "temp" | "t" => Namespace::Path(PathSpace::Temp),
        "context" | "c" => Namespace::Path(PathSpace::Context),
        "geometry" => Namespace::Resource(ResourceSpace::Geometry),
        "material" => Namespace::Resource(ResourceSpace::Material),
        "texture" => Namespace::Resource(ResourceSpace::Texture),
        "array" => Namespace::Resource(ResourceSpace::Array),
        _ => return None,
    })
}

fn is_lvalue(expr: &Expr) -> bool {
    match expr {
        Expr::Path { .. } => true,
        Expr::Arrow { reference, .. } => matches!(**reference, Expr::Path { .. }),
        _ => false,
    }
}

fn binary(op: Punct, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}
