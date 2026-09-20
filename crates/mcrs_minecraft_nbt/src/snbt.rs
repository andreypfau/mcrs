use std::fmt::Write;

use crate::Error;
use crate::compound::NbtCompound;
use crate::tag::NbtTag;

pub type Result<T> = std::result::Result<T, Error>;

pub fn parse_tag(input: &str) -> Result<NbtTag> {
    let mut parser = Parser { src: input, pos: 0 };
    let tag = parser.value()?;
    parser.skip_ws();
    if parser.pos < input.len() {
        return parser.err("trailing data");
    }
    Ok(tag)
}

pub fn parse_compound(input: &str) -> Result<NbtCompound> {
    match parse_tag(input)? {
        NbtTag::Compound(compound) => Ok(compound),
        other => Err(Error::Snbt {
            position: 0,
            message: format!("expected a compound, got {}", write(&other)),
        }),
    }
}

pub fn write(tag: &NbtTag) -> String {
    let mut out = String::new();
    write_tag(tag, &mut out);
    out
}

pub fn write_compound(compound: &NbtCompound) -> String {
    write(&NbtTag::Compound(compound.clone()))
}

fn write_tag(tag: &NbtTag, out: &mut String) {
    match tag {
        NbtTag::End => out.push_str("END"),
        NbtTag::Byte(v) => write!(out, "{v}b").unwrap(),
        NbtTag::Short(v) => write!(out, "{v}s").unwrap(),
        NbtTag::Int(v) => write!(out, "{v}").unwrap(),
        NbtTag::Long(v) => write!(out, "{v}L").unwrap(),
        NbtTag::Float(v) => write!(out, "{}f", java_float(*v)).unwrap(),
        NbtTag::Double(v) => write!(out, "{}d", java_double(*v)).unwrap(),
        NbtTag::String(v) => quote_and_escape(v, out),
        NbtTag::ByteArray(v) => {
            out.push_str("[B;");
            for (i, b) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write!(out, "{}B", *b as i8).unwrap();
            }
            out.push(']');
        }
        NbtTag::IntArray(v) => {
            out.push_str("[I;");
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write!(out, "{n}").unwrap();
            }
            out.push(']');
        }
        NbtTag::LongArray(v) => {
            out.push_str("[L;");
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write!(out, "{n}L").unwrap();
            }
            out.push(']');
        }
        NbtTag::List(v) => {
            out.push('[');
            for (i, tag) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_tag(tag, out);
            }
            out.push(']');
        }
        NbtTag::Compound(v) => {
            let mut entries: Vec<&(String, NbtTag)> = v.child_tags.iter().collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (i, (key, value)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if is_unquoted_key(key) {
                    out.push_str(key);
                } else {
                    quote_and_escape(key, out);
                }
                out.push(':');
                write_tag(value, out);
            }
            out.push('}');
        }
    }
}

fn is_unquoted_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '.' || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
        && !key.eq_ignore_ascii_case("true")
        && !key.eq_ignore_ascii_case("false")
}

fn quote_and_escape(input: &str, out: &mut String) {
    let mark = out.len();
    out.push(' ');
    let mut quote = None;
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' | '\'' => {
                let chosen = *quote.get_or_insert(if c == '"' { '\'' } else { '"' });
                if chosen == c {
                    out.push('\\');
                }
                out.push(c);
            }
            _ => match control_escape(c) {
                Some(escaped) => {
                    out.push('\\');
                    out.push_str(&escaped);
                }
                None => out.push(c),
            },
        }
    }
    let quote = quote.unwrap_or('"');
    out.replace_range(mark..mark + 1, quote.encode_utf8(&mut [0; 4]));
    out.push(quote);
}

fn control_escape(c: char) -> Option<String> {
    Some(match c {
        '\u{8}' => "b".to_string(),
        '\t' => "t".to_string(),
        '\n' => "n".to_string(),
        '\u{c}' => "f".to_string(),
        '\r' => "r".to_string(),
        c if c < ' ' => format!("x{:02X}", c as u32),
        _ => return None,
    })
}

/// Java's `Double.toString`: the shortest digits that round-trip but never
/// fewer than two, plain notation in `[1e-3, 1e7)` and `d.dddE±n` outside.
fn java_double(v: f64) -> String {
    let two_digits = format!("{v:.1e}");
    java_number(v, format!("{v:e}"), two_digits.parse() == Ok(v), two_digits)
}

fn java_float(v: f32) -> String {
    let two_digits = format!("{v:.1e}");
    java_number(
        v as f64,
        format!("{v:e}"),
        two_digits.parse() == Ok(v),
        two_digits,
    )
}

fn java_number(
    v: f64,
    shortest: String,
    two_digits_round_trip: bool,
    two_digits: String,
) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    let split = |text: &str| -> (String, i32) {
        let (mantissa, exponent) = text.split_once('e').unwrap();
        let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        while digits.len() > 1 && digits.ends_with('0') {
            digits.pop();
        }
        (digits, exponent.parse().unwrap())
    };
    let (mut digits, mut exponent) = split(&shortest);
    if digits.len() == 1 && two_digits_round_trip {
        (digits, exponent) = split(&two_digits);
    }
    let mut out = String::new();
    if v.is_sign_negative() {
        out.push('-');
    }
    let magnitude = v.abs();
    if magnitude == 0.0 {
        out.push_str("0.0");
    } else if (1e-3..1e7).contains(&magnitude) {
        if exponent >= 0 {
            let point = exponent as usize + 1;
            if digits.len() <= point {
                out.push_str(&digits);
                out.extend(std::iter::repeat_n('0', point - digits.len()));
                out.push_str(".0");
            } else {
                out.push_str(&digits[..point]);
                out.push('.');
                out.push_str(&digits[point..]);
            }
        } else {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        out.push('.');
        if digits.len() > 1 {
            out.push_str(&digits[1..]);
        } else {
            out.push('0');
        }
        write!(out, "E{exponent}").unwrap();
    }
    out
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum IntType {
    Byte,
    Short,
    Int,
    Long,
}

#[derive(Clone, Copy)]
struct IntSuffix {
    signed: Option<bool>,
    ty: Option<IntType>,
}

struct IntLiteral<'a> {
    negative: bool,
    radix: u32,
    digits: &'a str,
    suffix: IntSuffix,
}

fn is_java_whitespace(c: char) -> bool {
    matches!(c, '\u{1c}'..='\u{1f}')
        || (c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{2007}' | '\u{202f}'))
}

fn can_start_number(c: char) -> bool {
    matches!(c, '+' | '-' | '.' | '0'..='9')
}

fn is_unquoted_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+')
}

impl<'a> Parser<'a> {
    fn err<T>(&self, message: impl Into<String>) -> Result<T> {
        Err(Error::Snbt {
            position: self.pos,
            message: message.into(),
        })
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(is_java_whitespace) {
            self.bump();
        }
    }

    fn eat(&mut self, expected: char) -> bool {
        self.eat_if(|c| c == expected).is_some()
    }

    fn eat_if(&mut self, accept: impl Fn(char) -> bool) -> Option<char> {
        self.skip_ws();
        let c = self.peek().filter(|&c| accept(c))?;
        self.bump();
        Some(c)
    }

    fn expect(&mut self, expected: char) -> Result<()> {
        if self.eat(expected) {
            Ok(())
        } else {
            self.err(format!("expected '{expected}'"))
        }
    }

    fn value(&mut self) -> Result<NbtTag> {
        self.skip_ws();
        match self.peek() {
            Some(c) if can_start_number(c) => self.number(),
            Some('"' | '\'') => self.quoted_string().map(NbtTag::String),
            Some('{') => self.compound(),
            Some('[') => self.list(),
            _ => self.unquoted_or_builtin(),
        }
    }

    fn compound(&mut self) -> Result<NbtTag> {
        self.expect('{')?;
        let mut compound = NbtCompound::new();
        loop {
            if self.eat('}') {
                return Ok(NbtTag::Compound(compound));
            }
            self.skip_ws();
            let key = if matches!(self.peek(), Some('"' | '\'')) {
                self.quoted_string()?
            } else {
                self.unquoted_string()?.to_string()
            };
            if key.is_empty() {
                return self.err("empty key");
            }
            self.expect(':')?;
            let value = self.value()?;
            match compound.child_tags.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => compound.child_tags.push((key, value)),
            }
            if !self.eat(',') {
                self.expect('}')?;
                return Ok(NbtTag::Compound(compound));
            }
        }
    }

    fn list(&mut self) -> Result<NbtTag> {
        self.expect('[')?;
        let after_bracket = self.pos;
        if let Some(prefix) = self.eat_if(|c| matches!(c, 'B' | 'I' | 'L')) {
            if self.eat(';') {
                return self.array(prefix);
            }
            self.pos = after_bracket;
        }
        let mut elements = Vec::new();
        loop {
            if self.eat(']') {
                return Ok(NbtTag::List(elements));
            }
            elements.push(self.value()?);
            if !self.eat(',') {
                self.expect(']')?;
                return Ok(NbtTag::List(elements));
            }
        }
    }

    fn array(&mut self, prefix: char) -> Result<NbtTag> {
        let (default, allowed): (IntType, &[IntType]) = match prefix {
            'B' => (IntType::Byte, &[IntType::Byte]),
            'I' => (IntType::Int, &[IntType::Int, IntType::Byte, IntType::Short]),
            _ => (
                IntType::Long,
                &[IntType::Long, IntType::Byte, IntType::Short, IntType::Int],
            ),
        };
        let mut values = Vec::new();
        loop {
            if self.eat(']') {
                break;
            }
            let literal = self.integer_literal()?;
            let ty = literal.suffix.ty.unwrap_or(default);
            if !allowed.contains(&ty) {
                return self.err("invalid array element type");
            }
            values.push(self.integer_value(&literal, ty)?);
            if !self.eat(',') {
                self.expect(']')?;
                break;
            }
        }
        Ok(match prefix {
            'B' => NbtTag::ByteArray(values.into_iter().map(|v| v as u8).collect()),
            'I' => NbtTag::IntArray(values.into_iter().map(|v| v as i32).collect()),
            _ => NbtTag::LongArray(values),
        })
    }

    fn number(&mut self) -> Result<NbtTag> {
        let start = self.pos;
        if let Some(float) = self.float_literal()? {
            return Ok(float);
        }
        self.pos = start;
        let literal = self.integer_literal()?;
        let ty = literal.suffix.ty.unwrap_or(IntType::Int);
        let value = self.integer_value(&literal, ty)?;
        Ok(match ty {
            IntType::Byte => NbtTag::Byte(value as i8),
            IntType::Short => NbtTag::Short(value as i16),
            IntType::Int => NbtTag::Int(value as i32),
            IntType::Long => NbtTag::Long(value),
        })
    }

    fn sign(&mut self) -> bool {
        match self.eat_if(|c| c == '+' || c == '-') {
            Some(c) => c == '-',
            None => false,
        }
    }

    /// `None` when there is no run or an underscore sits at either end, which
    /// the grammar rejects rather than trims.
    fn digit_run(&mut self, accept: impl Fn(char) -> bool) -> Option<&'a str> {
        self.skip_ws();
        let start = self.pos;
        while self.peek().is_some_and(|c| c == '_' || accept(c)) {
            self.bump();
        }
        let run = &self.src[start..self.pos];
        if run.is_empty() || run.starts_with('_') || run.ends_with('_') {
            self.pos = start;
            return None;
        }
        Some(run)
    }

    fn decimal_run(&mut self) -> Option<&'a str> {
        self.digit_run(|c| c.is_ascii_digit())
    }

    fn float_literal(&mut self) -> Result<Option<NbtTag>> {
        let negative = self.sign();
        let whole = self.decimal_run();
        let (fraction, exponent) = if self.eat('.') {
            if whole.is_some() {
                (self.decimal_run(), self.exponent()?)
            } else {
                let Some(run) = self.decimal_run() else {
                    return self.err("expected a decimal number");
                };
                (Some(run), self.exponent()?)
            }
        } else if whole.is_some() {
            let exponent = self.exponent()?;
            if exponent.is_none() && !self.peek_float_suffix() {
                return Ok(None);
            }
            (None, exponent)
        } else {
            return Ok(None);
        };
        let suffix = self.eat_if(|c| matches!(c, 'f' | 'F' | 'd' | 'D'));
        let mut text = String::new();
        if negative {
            text.push('-');
        }
        text.extend(whole.unwrap_or("0").chars().filter(|&c| c != '_'));
        text.push('.');
        text.extend(fraction.unwrap_or("0").chars().filter(|&c| c != '_'));
        if let Some((exp_negative, exp_digits)) = exponent {
            text.push('e');
            if exp_negative {
                text.push('-');
            }
            text.extend(exp_digits.chars().filter(|&c| c != '_'));
        }
        // -0.0 parses as +0.0: the reference's float tags fold the sign away.
        let tag = if matches!(suffix, Some('f' | 'F')) {
            let value: f32 = text.parse().map_err(|e| self.error(format!("{e}")))?;
            if !value.is_finite() {
                return self.err("infinity is not allowed");
            }
            NbtTag::Float(value + 0.0)
        } else {
            let value: f64 = text.parse().map_err(|e| self.error(format!("{e}")))?;
            if !value.is_finite() {
                return self.err("infinity is not allowed");
            }
            NbtTag::Double(value + 0.0)
        };
        Ok(Some(tag))
    }

    fn peek_float_suffix(&mut self) -> bool {
        self.skip_ws();
        self.peek()
            .is_some_and(|c| matches!(c, 'f' | 'F' | 'd' | 'D'))
    }

    fn error(&self, message: String) -> Error {
        Error::Snbt {
            position: self.pos,
            message,
        }
    }

    fn exponent(&mut self) -> Result<Option<(bool, &'a str)>> {
        let start = self.pos;
        if self.eat_if(|c| c == 'e' || c == 'E').is_none() {
            return Ok(None);
        }
        let negative = self.sign();
        match self.decimal_run() {
            Some(digits) => Ok(Some((negative, digits))),
            None => {
                self.pos = start;
                Ok(None)
            }
        }
    }

    fn integer_literal(&mut self) -> Result<IntLiteral<'a>> {
        let negative = self.sign();
        let (radix, digits) = if self.eat('0') {
            if self.eat_if(|c| c == 'x' || c == 'X').is_some() {
                let Some(run) = self.digit_run(|c| c.is_ascii_hexdigit()) else {
                    return self.err("expected a hexadecimal number");
                };
                (16, run)
            } else {
                let before_b = self.pos;
                match self.eat_if(|c| c == 'b' || c == 'B') {
                    Some(_) => match self.digit_run(|c| c == '0' || c == '1') {
                        Some(run) => (2, run),
                        None => {
                            self.pos = before_b;
                            (10, "0")
                        }
                    },
                    None => {
                        if self.decimal_run().is_some() {
                            return self.err("leading zero is not allowed");
                        }
                        (10, "0")
                    }
                }
            }
        } else {
            let Some(run) = self.decimal_run() else {
                return self.err("expected a decimal number");
            };
            (10, run)
        };
        let suffix = self.integer_suffix();
        Ok(IntLiteral {
            negative,
            radix,
            digits,
            suffix,
        })
    }

    fn integer_suffix(&mut self) -> IntSuffix {
        let start = self.pos;
        let ty = |c: char| match c.to_ascii_lowercase() {
            'b' => Some(IntType::Byte),
            's' => Some(IntType::Short),
            'i' => Some(IntType::Int),
            'l' => Some(IntType::Long),
            _ => None,
        };
        if let Some(prefix) = self.eat_if(|c| matches!(c, 'u' | 'U' | 's' | 'S')) {
            if let Some(t) = self.eat_if(|c| ty(c).is_some()) {
                return IntSuffix {
                    signed: Some(prefix.eq_ignore_ascii_case(&'s')),
                    ty: ty(t),
                };
            }
            self.pos = start;
        }
        match self.eat_if(|c| ty(c).is_some()) {
            Some(t) => IntSuffix {
                signed: None,
                ty: ty(t),
            },
            None => IntSuffix {
                signed: None,
                ty: None,
            },
        }
    }

    fn integer_value(&self, literal: &IntLiteral, ty: IntType) -> Result<i64> {
        let signed = literal.suffix.signed.unwrap_or(literal.radix == 10);
        if !signed && literal.negative {
            return self.err("expected a non-negative number");
        }
        let mut digits: String = literal.digits.chars().filter(|&c| c != '_').collect();
        if literal.negative {
            digits.insert(0, '-');
        }
        let radix = literal.radix;
        let out_of_range = || self.error(format!("number out of range: {digits}"));
        Ok(match (signed, ty) {
            (true, IntType::Byte) => {
                i8::from_str_radix(&digits, radix).map_err(|_| out_of_range())? as i64
            }
            (true, IntType::Short) => {
                i16::from_str_radix(&digits, radix).map_err(|_| out_of_range())? as i64
            }
            (true, IntType::Int) => {
                i32::from_str_radix(&digits, radix).map_err(|_| out_of_range())? as i64
            }
            (true, IntType::Long) => {
                i64::from_str_radix(&digits, radix).map_err(|_| out_of_range())?
            }
            (false, IntType::Byte) => {
                u8::from_str_radix(&digits, radix).map_err(|_| out_of_range())? as i8 as i64
            }
            (false, IntType::Short) => {
                u16::from_str_radix(&digits, radix).map_err(|_| out_of_range())? as i16 as i64
            }
            (false, IntType::Int) => {
                u32::from_str_radix(&digits, radix).map_err(|_| out_of_range())? as i32 as i64
            }
            (false, IntType::Long) => {
                u64::from_str_radix(&digits, radix).map_err(|_| out_of_range())? as i64
            }
        })
    }

    fn quoted_string(&mut self) -> Result<String> {
        self.skip_ws();
        let quote = self.bump().unwrap();
        let mut out = String::new();
        loop {
            let Some(c) = self.bump() else {
                return self.err("unterminated string");
            };
            if c == quote {
                return Ok(out);
            }
            if c != '\\' {
                out.push(c);
                continue;
            }
            match self.bump() {
                Some('b') => out.push('\u{8}'),
                Some('s') => out.push(' '),
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('f') => out.push('\u{c}'),
                Some('r') => out.push('\r'),
                Some(c @ ('\\' | '\'' | '"')) => out.push(c),
                Some('x') => out.push(self.hex_escape(2)?),
                Some('u') => out.push(self.hex_escape(4)?),
                Some('U') => out.push(self.hex_escape(8)?),
                Some('N') => return self.err("named character escapes are not supported"),
                _ => return self.err("invalid escape sequence"),
            }
        }
    }

    fn hex_escape(&mut self, len: usize) -> Result<char> {
        let start = self.pos;
        let hex = self.src[start..].get(..len);
        match hex {
            Some(hex) if hex.chars().all(|c| c.is_ascii_hexdigit()) => {
                self.pos += len;
                u32::from_str_radix(hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| self.error(format!("invalid Unicode character value: {hex}")))
            }
            _ => self.err(format!("expected a character literal of length {len}")),
        }
    }

    fn unquoted_string(&mut self) -> Result<&'a str> {
        self.skip_ws();
        let start = self.pos;
        while self.peek().is_some_and(is_unquoted_char) {
            self.bump();
        }
        if self.pos == start {
            return self.err("expected a valid unquoted string");
        }
        Ok(&self.src[start..self.pos])
    }

    fn unquoted_or_builtin(&mut self) -> Result<NbtTag> {
        let contents = self.unquoted_string()?;
        if contents.starts_with(can_start_number) {
            return self.err("invalid start of an unquoted string");
        }
        if self.eat('(') {
            let mut arguments = Vec::new();
            loop {
                if self.eat(')') {
                    break;
                }
                arguments.push(self.value()?);
                if !self.eat(',') {
                    self.expect(')')?;
                    break;
                }
            }
            return self.builtin(contents, arguments);
        }
        Ok(if contents.eq_ignore_ascii_case("true") {
            NbtTag::Byte(1)
        } else if contents.eq_ignore_ascii_case("false") {
            NbtTag::Byte(0)
        } else {
            NbtTag::String(contents.to_string())
        })
    }

    fn builtin(&self, name: &str, arguments: Vec<NbtTag>) -> Result<NbtTag> {
        match (name, arguments.as_slice()) {
            ("bool", [argument]) => {
                let truthy = match argument {
                    NbtTag::Byte(v) => *v != 0,
                    NbtTag::Short(v) => *v != 0,
                    NbtTag::Int(v) => *v != 0,
                    NbtTag::Long(v) => *v != 0,
                    NbtTag::Float(v) => *v != 0.0,
                    NbtTag::Double(v) => *v != 0.0,
                    _ => return self.err("expected a number or a boolean"),
                };
                Ok(NbtTag::Byte(truthy as i8))
            }
            ("uuid", [NbtTag::String(text)]) => parse_uuid(text)
                .map(|(most, least)| {
                    NbtTag::IntArray(vec![
                        (most >> 32) as i32,
                        most as i32,
                        (least >> 32) as i32,
                        least as i32,
                    ])
                })
                .ok_or_else(|| {
                    self.error("expected a string representing a valid UUID".to_string())
                }),
            ("uuid", [_]) => self.err("expected a string representing a valid UUID"),
            _ => self.err(format!("no such operation: {name}/{}", arguments.len())),
        }
    }
}

/// `java.util.UUID.fromString`: five dash-separated hex groups, each group
/// masked to its field width, at most 36 characters in total.
fn parse_uuid(text: &str) -> Option<(i64, i64)> {
    if text.len() > 36 {
        return None;
    }
    let groups: Vec<&str> = text.split('-').collect();
    let [a, b, c, d, e] = groups.as_slice() else {
        return None;
    };
    let hex = |group: &str| -> Option<i64> {
        if group.is_empty() || group.len() > 16 {
            return None;
        }
        i64::from_str_radix(group, 16).ok()
    };
    let most = ((hex(a)? & 0xffff_ffff) << 32) | ((hex(b)? & 0xffff) << 16) | (hex(c)? & 0xffff);
    let least = ((hex(d)? & 0xffff) << 48) | (hex(e)? & 0xffff_ffff_ffff);
    Some((most, least))
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::deserializer::NbtReadHelper;
    use crate::serializer::WriteAdaptor;
    use crate::snbt_golden::*;

    fn hex(tag: &NbtTag) -> String {
        let mut bytes = Vec::new();
        tag.write_unnamed(&mut WriteAdaptor::new(&mut bytes))
            .unwrap();
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn unhex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    fn read(bytes: &[u8]) -> NbtTag {
        NbtTag::read_unnamed(&mut NbtReadHelper::new(Cursor::new(bytes))).unwrap()
    }

    /// Vanilla's compound is a hash map, so its bytes carry no key order; the
    /// tag decoded from them is compared through the key-sorted SNBT form.
    #[test]
    fn parses_like_vanilla() {
        for (input, expected_hex, expected_snbt) in PARSES {
            let tag = parse_tag(input).unwrap_or_else(|e| panic!("{input:?}: {e}"));
            assert_eq!(write(&tag), *expected_snbt, "snbt of {input:?}");
            assert_eq!(
                write(&read(&unhex(expected_hex))),
                *expected_snbt,
                "bytes of {input:?}"
            );
            assert_eq!(
                write(&parse_tag(expected_snbt).unwrap()),
                *expected_snbt,
                "round trip of {input:?}"
            );
        }
    }

    #[test]
    fn rejects_like_vanilla() {
        for input in REJECTS {
            assert!(parse_tag(input).is_err(), "{input:?} should not parse");
        }
    }

    #[test]
    fn formats_numbers_like_java() {
        for (bits, expected) in DOUBLES {
            assert_eq!(java_double(f64::from_bits(*bits)), *expected);
        }
        for (bits, expected) in FLOATS {
            assert_eq!(java_float(f32::from_bits(*bits)), *expected);
        }
    }

    #[test]
    fn writes_like_vanilla_and_round_trips_its_bytes() {
        let tag = read(&unhex(PRETTY_HEX));
        assert_eq!(write(&tag), PRETTY);
        assert_eq!(hex(&tag), PRETTY_HEX);
    }

    #[test]
    fn a_compound_is_required_by_parse_compound() {
        assert!(parse_compound("{a:1}").is_ok());
        assert!(parse_compound("[1]").is_err());
        assert!(parse_compound("\"{}\"").is_err());
    }
}
