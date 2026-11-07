use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Punct {
    Bang,
    Minus,
    Plus,
    Star,
    Slash,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    EqEq,
    NotEq,
    AndAnd,
    OrOr,
    Question,
    Colon,
    Coalesce,
    Assign,
    Arrow,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Dot,
}

impl Punct {
    pub fn text(self) -> &'static str {
        use Punct::*;
        match self {
            Bang => "!",
            Minus => "-",
            Plus => "+",
            Star => "*",
            Slash => "/",
            Less => "<",
            LessEq => "<=",
            Greater => ">",
            GreaterEq => ">=",
            EqEq => "==",
            NotEq => "!=",
            AndAnd => "&&",
            OrOr => "||",
            Question => "?",
            Colon => ":",
            Coalesce => "??",
            Assign => "=",
            Arrow => "->",
            LParen => "(",
            RParen => ")",
            LBrace => "{",
            RBrace => "}",
            LBracket => "[",
            RBracket => "]",
            Comma => ",",
            Semi => ";",
            Dot => ".",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Number(f64),
    String(String),
    /// A name, folded to lower case.
    Name(String),
    Punct(Punct),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub offset: usize,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Number(value) => write!(formatter, "{value}"),
            TokenKind::String(text) => write!(formatter, "'{text}'"),
            TokenKind::Name(name) => formatter.write_str(name),
            TokenKind::Punct(punct) => formatter.write_str(punct.text()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LexErrorKind {
    NonAscii,
    UnknownChar(char),
    UnterminatedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub offset: usize,
}

pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut pos = 0;
    while let Some(&byte) = bytes.get(pos) {
        let start = pos;
        let next = bytes.get(pos + 1).copied();
        let kind = match byte {
            b' ' | b'\t' | b'\r' | b'\n' => {
                pos += 1;
                continue;
            }
            b'0'..=b'9' => number(bytes, &mut pos),
            b'.' if next.is_some_and(|byte| byte.is_ascii_digit()) => number(bytes, &mut pos),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while bytes
                    .get(pos)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    pos += 1;
                }
                let text = source.get(start..pos).unwrap_or_default();
                TokenKind::Name(text.to_ascii_lowercase())
            }
            b'\'' => {
                let Some(length) = bytes
                    .get(pos + 1..)
                    .and_then(|rest| rest.iter().position(|byte| *byte == b'\''))
                else {
                    return Err(LexError {
                        kind: LexErrorKind::UnterminatedString,
                        offset: start,
                    });
                };
                let text = source.get(pos + 1..pos + 1 + length).unwrap_or_default();
                pos += length + 2;
                TokenKind::String(text.to_owned())
            }
            _ => {
                let (punct, length) = punct(byte, next).ok_or_else(|| LexError {
                    kind: if byte.is_ascii() {
                        LexErrorKind::UnknownChar(char::from(byte))
                    } else {
                        LexErrorKind::NonAscii
                    },
                    offset: start,
                })?;
                pos += length;
                TokenKind::Punct(punct)
            }
        };
        tokens.push(Token {
            kind,
            offset: start,
        });
    }
    Ok(tokens)
}

fn number(bytes: &[u8], pos: &mut usize) -> TokenKind {
    let start = *pos;
    let digits = |pos: &mut usize| {
        while bytes.get(*pos).is_some_and(u8::is_ascii_digit) {
            *pos += 1;
        }
    };
    digits(pos);
    if bytes.get(*pos) == Some(&b'.') && bytes.get(*pos + 1).is_some_and(u8::is_ascii_digit) {
        *pos += 1;
        digits(pos);
    }
    let text = bytes.get(start..*pos).unwrap_or_default();
    let value = std::str::from_utf8(text)
        .ok()
        .and_then(|text| text.parse::<f64>().ok())
        .unwrap_or(0.0);
    if matches!(bytes.get(*pos), Some(b'f' | b'F'))
        && !bytes
            .get(*pos + 1)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        *pos += 1;
    }
    TokenKind::Number(value)
}

fn punct(byte: u8, next: Option<u8>) -> Option<(Punct, usize)> {
    use Punct::*;
    let pair = match (byte, next) {
        (b'<', Some(b'=')) => Some(LessEq),
        (b'>', Some(b'=')) => Some(GreaterEq),
        (b'=', Some(b'=')) => Some(EqEq),
        (b'!', Some(b'=')) => Some(NotEq),
        (b'&', Some(b'&')) => Some(AndAnd),
        (b'|', Some(b'|')) => Some(OrOr),
        (b'?', Some(b'?')) => Some(Coalesce),
        (b'-', Some(b'>')) => Some(Arrow),
        _ => None,
    };
    if let Some(punct) = pair {
        return Some((punct, 2));
    }
    let single = match byte {
        b'!' => Bang,
        b'-' => Minus,
        b'+' => Plus,
        b'*' => Star,
        b'/' => Slash,
        b'<' => Less,
        b'>' => Greater,
        b'?' => Question,
        b':' => Colon,
        b'=' => Assign,
        b'(' => LParen,
        b')' => RParen,
        b'{' => LBrace,
        b'}' => RBrace,
        b'[' => LBracket,
        b']' => RBracket,
        b',' => Comma,
        b';' => Semi,
        b'.' => Dot,
        _ => return None,
    };
    Some((single, 1))
}
