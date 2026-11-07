use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::diagnostic::{Diagnostic, ErrorKind, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Punct {
    Bang,
    Minus,
    Plus,
    Star,
    Slash,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    EqualEqual,
    BangEqual,
    AndAnd,
    OrOr,
    Question,
    Colon,
    QuestionQuestion,
    Equal,
    Arrow,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    Comma,
    Semicolon,
    Dot,
}

impl Punct {
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Bang => "!",
            Self::Minus => "-",
            Self::Plus => "+",
            Self::Star => "*",
            Self::Slash => "/",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::EqualEqual => "==",
            Self::BangEqual => "!=",
            Self::AndAnd => "&&",
            Self::OrOr => "||",
            Self::Question => "?",
            Self::Colon => ":",
            Self::QuestionQuestion => "??",
            Self::Equal => "=",
            Self::Arrow => "->",
            Self::OpenParen => "(",
            Self::CloseParen => ")",
            Self::OpenBrace => "{",
            Self::CloseBrace => "}",
            Self::OpenBracket => "[",
            Self::CloseBracket => "]",
            Self::Comma => ",",
            Self::Semicolon => ";",
            Self::Dot => ".",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TokenKind {
    Number(f32),
    /// The text between the quotes, with its case.
    String(Box<str>),
    /// A name, folded to lower case.
    Name(Box<str>),
    Punct(Punct),
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

const PUNCTS: [(&str, Punct); 27] = [
    ("??", Punct::QuestionQuestion),
    ("<=", Punct::LessEqual),
    (">=", Punct::GreaterEqual),
    ("==", Punct::EqualEqual),
    ("!=", Punct::BangEqual),
    ("&&", Punct::AndAnd),
    ("||", Punct::OrOr),
    ("->", Punct::Arrow),
    ("!", Punct::Bang),
    ("-", Punct::Minus),
    ("+", Punct::Plus),
    ("*", Punct::Star),
    ("/", Punct::Slash),
    ("<", Punct::Less),
    (">", Punct::Greater),
    ("?", Punct::Question),
    (":", Punct::Colon),
    ("=", Punct::Equal),
    ("(", Punct::OpenParen),
    (")", Punct::CloseParen),
    ("{", Punct::OpenBrace),
    ("}", Punct::CloseBrace),
    ("[", Punct::OpenBracket),
    ("]", Punct::CloseBracket),
    (",", Punct::Comma),
    (";", Punct::Semicolon),
    (".", Punct::Dot),
];

/// Splits the text into tokens. The last token is always [`TokenKind::End`].
pub(crate) fn lex(source: &str, diagnostics: &mut Vec<Diagnostic>) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let start = at;
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' => {
                at += 1;
                continue;
            }
            b'0'..=b'9' => at = number_end(bytes, at),
            b'.' if bytes.get(at + 1).is_some_and(u8::is_ascii_digit) => {
                at = number_end(bytes, at);
            }
            b'\'' => {
                let Some(length) = source.get(at + 1..).and_then(|rest| rest.find('\'')) else {
                    diagnostics.push(Diagnostic::new(
                        ErrorKind::Syntax,
                        Span::new(at..source.len()),
                        "unterminated string literal",
                    ));
                    break;
                };
                let text = source.get(at + 1..at + 1 + length).unwrap_or_default();
                at += length + 2;
                tokens.push(Token {
                    kind: TokenKind::String(text.into()),
                    span: Span::new(start..at),
                });
                continue;
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while bytes
                    .get(at)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    at += 1;
                }
                let name: String = source
                    .get(start..at)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                tokens.push(Token {
                    kind: TokenKind::Name(name.into_boxed_str()),
                    span: Span::new(start..at),
                });
                continue;
            }
            _ => {
                let rest = source.get(at..).unwrap_or_default();
                if let Some((text, punct)) = PUNCTS.iter().find(|(text, _)| rest.starts_with(text))
                {
                    at += text.len();
                    tokens.push(Token {
                        kind: TokenKind::Punct(*punct),
                        span: Span::new(start..at),
                    });
                } else {
                    let width = rest.chars().next().map_or(1, char::len_utf8);
                    at += width;
                    diagnostics.push(Diagnostic::new(
                        ErrorKind::Syntax,
                        Span::new(start..at),
                        "a character that is not part of a token",
                    ));
                }
                continue;
            }
        }
        let text = source.get(start..at).unwrap_or_default();
        let digits = text.strip_suffix(['f', 'F']).unwrap_or(text);
        let value: f32 = digits.parse().unwrap_or(0.0);
        tokens.push(Token {
            kind: TokenKind::Number(value),
            span: Span::new(start..at),
        });
    }
    tokens.push(Token {
        kind: TokenKind::End,
        span: Span::new(source.len()..source.len()),
    });
    tokens
}

fn number_end(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    if bytes.get(at) == Some(&b'.') && bytes.get(at + 1).is_some_and(u8::is_ascii_digit) {
        at += 1;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
    }
    if matches!(bytes.get(at), Some(b'f' | b'F'))
        && !bytes
            .get(at + 1)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        at += 1;
    }
    at
}
