use super::build::{ArrayKind, Build, Scalar};
use super::error::{IntegerType, SnbtError, SnbtErrorKind as K, Span};
use super::text::{
    ESCAPE_LETTERS, StrText, char_by_name, hex_digits, hex_length, is_high_surrogate,
    is_low_surrogate, is_name_char, is_number_start, is_unquoted, is_whitespace, simple_escape,
};
use crate::MAX_DEPTH;

pub(crate) enum Fail {
    Soft,
    Abort(SnbtError),
}

type R<T> = Result<T, Fail>;

/// What an operation and the compound read need to know about a value, whatever the builder.
#[derive(Clone, Copy)]
pub(crate) enum Shape<'a> {
    Number { nonzero: bool },
    Str(StrText<'a>),
    Compound,
    Other,
}

#[derive(Clone, Copy)]
struct Suffix {
    signed: Option<bool>,
    ty: IntegerType,
}

#[derive(Clone, Copy)]
struct IntLiteral<'a> {
    negative: bool,
    radix: u32,
    digits: &'a str,
    suffix: Option<Suffix>,
    span: Span,
}

struct FloatParts<'a> {
    whole: Option<&'a str>,
    fraction: Option<&'a str>,
    exponent: Option<(bool, &'a str)>,
    float: bool,
}

pub(crate) struct Reader<'a, B> {
    input: &'a str,
    pos: usize,
    best: Option<SnbtError>,
    build: B,
}

impl<'a, B: Build> Reader<'a, B> {
    pub(crate) fn new(input: &'a str, start: usize, build: B) -> Self {
        Self {
            input,
            pos: start,
            best: None,
            build,
        }
    }

    pub(crate) fn position(&self) -> usize {
        self.pos
    }

    pub(crate) fn rest(&self) -> &'a str {
        self.input.get(self.pos..).unwrap_or_default()
    }

    /// One tag from the cursor (SNBT-GR-001); the error is the first record at the furthest
    /// position (SNBT-ERR-002).
    pub(crate) fn read(&mut self) -> Result<(B::Value, Shape<'a>), SnbtError> {
        match self.value(1) {
            Ok(value) => Ok(value),
            Err(Fail::Abort(error)) => Err(error),
            Err(Fail::Soft) => Err(self.best.unwrap_or(SnbtError {
                kind: K::ExpectedUnquotedString,
                position: self.pos,
            })),
        }
    }

    fn record(&mut self, position: usize, kind: K) {
        if self.best.is_none_or(|best| position > best.position) {
            self.best = Some(SnbtError { kind, position });
        }
    }

    fn fail<T>(&mut self, position: usize, kind: K) -> R<T> {
        self.record(position, kind);
        Err(Fail::Soft)
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    pub(crate) fn skip_whitespace(&mut self) {
        let rest = self.rest();
        self.pos += rest.len() - rest.trim_start_matches(is_whitespace).len();
    }

    fn peek_token(&self) -> Option<char> {
        self.rest().trim_start_matches(is_whitespace).chars().next()
    }

    /// A character token: one code unit of `set`, whose members are joined with `|`.
    fn token(&mut self, set: &'static str) -> R<char> {
        let start = self.pos;
        self.skip_whitespace();
        match self.peek() {
            Some(c) if c != '|' && set.contains(c) => {
                self.pos += c.len_utf8();
                Ok(c)
            }
            _ => {
                let at = self.pos;
                self.pos = start;
                self.fail(at, K::LiteralIncorrect(set))
            }
        }
    }

    fn opt<T>(&mut self, read: impl FnOnce(&mut Self) -> R<T>) -> R<Option<T>> {
        let start = self.pos;
        match read(self) {
            Ok(value) => Ok(Some(value)),
            Err(Fail::Soft) => {
                self.pos = start;
                Ok(None)
            }
            Err(abort) => Err(abort),
        }
    }

    fn first_token(&mut self, sets: &[&'static str]) -> R<char> {
        for set in sets {
            if let Some(c) = self.opt(|r| r.token(set))? {
                return Ok(c);
            }
        }
        Err(Fail::Soft)
    }

    fn run(&mut self, accept: fn(char) -> bool, empty: K) -> R<&'a str> {
        let start = self.pos;
        self.skip_whitespace();
        let at = self.pos;
        let rest = self.rest();
        let length = rest
            .find(|c: char| !(c == '_' || accept(c)))
            .unwrap_or(rest.len());
        let run = rest.get(..length).unwrap_or_default();
        let kind = if run.is_empty() {
            empty
        } else if run.starts_with('_') || run.ends_with('_') {
            K::UnderscoreNotAllowed
        } else {
            self.pos = at + length;
            return Ok(run);
        };
        self.pos = start;
        self.fail(at, kind)
    }

    fn decimal(&mut self) -> R<&'a str> {
        self.run(|c| c.is_ascii_digit(), K::ExpectedDecimalNumeral)
    }

    fn word(&mut self) -> R<&'a str> {
        let start = self.pos;
        self.skip_whitespace();
        let at = self.pos;
        let rest = self.rest();
        let length = rest.find(|c| !is_unquoted(c)).unwrap_or(rest.len());
        match rest.get(..length) {
            Some(word) if !word.is_empty() => {
                self.pos = at + length;
                Ok(word)
            }
            _ => {
                self.pos = start;
                self.fail(at, K::ExpectedUnquotedString)
            }
        }
    }

    /// `[ item { ',' item } [ ',' ] ]` (SNBT-GR-009).
    fn items(&mut self, mut item: impl FnMut(&mut Self) -> R<()>) -> R<()> {
        if self.opt(&mut item)?.is_none() {
            return Ok(());
        }
        loop {
            let start = self.pos;
            if self.opt(|r| r.token(","))?.is_none() {
                break;
            }
            if self.opt(&mut item)?.is_none() {
                self.pos = start;
                break;
            }
        }
        self.opt(|r| r.token(","))?;
        Ok(())
    }

    fn value(&mut self, depth: u32) -> R<(B::Value, Shape<'a>)> {
        match self.peek_token() {
            Some(c) if is_number_start(c) => {
                let start = self.pos;
                if let Some(value) = self.opt(Self::float)? {
                    return Ok(value);
                }
                if let Some(literal) = self.opt(Self::integer)? {
                    let (scalar, nonzero) = self.check_integer(literal)?;
                    return Ok((self.build.scalar(scalar), Shape::Number { nonzero }));
                }
                self.pos = start;
                self.word_value(depth)
            }
            Some('"' | '\'') => {
                let text = self.quoted()?;
                Ok((self.build.string(text), Shape::Str(text)))
            }
            Some('{') => self.compound(depth),
            Some('[') => self.list(depth),
            _ => self.word_value(depth),
        }
    }

    fn sign(&mut self) -> R<bool> {
        Ok(self.opt(|r| r.first_token(&["+", "-"]))? == Some('-'))
    }

    fn exponent(&mut self) -> R<(bool, &'a str)> {
        self.token("e|E")?;
        let negative = self.sign()?;
        Ok((negative, self.decimal()?))
    }

    fn float_suffix(&mut self) -> R<bool> {
        Ok(self
            .first_token(&["f|F", "d|D"])?
            .eq_ignore_ascii_case(&'f'))
    }

    fn float_tail(
        &mut self,
        whole: Option<&'a str>,
        fraction: Option<&'a str>,
    ) -> R<FloatParts<'a>> {
        let exponent = self.opt(Self::exponent)?;
        let float = self.opt(Self::float_suffix)? == Some(true);
        Ok(FloatParts {
            whole,
            fraction,
            exponent,
            float,
        })
    }

    fn float_body(&mut self) -> R<FloatParts<'a>> {
        let start = self.pos;
        if let Some(whole) = self.opt(Self::decimal)?
            && self.opt(|r| r.token("."))?.is_some()
        {
            let fraction = self.opt(Self::decimal)?;
            return self.float_tail(Some(whole), fraction);
        }
        self.pos = start;
        if self.opt(|r| r.token("."))?.is_some() {
            let fraction = self.decimal()?;
            return self.float_tail(None, Some(fraction));
        }
        self.pos = start;
        let whole = self.decimal()?;
        let exponent = self.opt(Self::exponent)?;
        let float = if exponent.is_some() {
            self.opt(Self::float_suffix)? == Some(true)
        } else {
            self.float_suffix()?
        };
        Ok(FloatParts {
            whole: Some(whole),
            fraction: None,
            exponent,
            float,
        })
    }

    fn float(&mut self) -> R<(B::Value, Shape<'a>)> {
        let negative = self.sign()?;
        let parts = self.float_body()?;
        match float_value(negative, &parts) {
            Some((scalar, nonzero)) => Ok((self.build.scalar(scalar), Shape::Number { nonzero })),
            None => self.fail(self.pos, K::InfinityNotAllowed),
        }
    }

    fn type_letter(&mut self) -> R<IntegerType> {
        Ok(match self.first_token(&["b|B", "s|S", "i|I", "l|L"])? {
            'b' | 'B' => IntegerType::Byte,
            's' | 'S' => IntegerType::Short,
            'i' | 'I' => IntegerType::Int,
            _ => IntegerType::Long,
        })
    }

    fn integer_suffix(&mut self) -> R<Option<Suffix>> {
        for (mark, signed) in [("u|U", false), ("s|S", true)] {
            let start = self.pos;
            if self.opt(|r| r.token(mark))?.is_some() {
                if let Some(ty) = self.opt(Self::type_letter)? {
                    return Ok(Some(Suffix {
                        signed: Some(signed),
                        ty,
                    }));
                }
                self.pos = start;
            }
        }
        Ok(self
            .opt(Self::type_letter)?
            .map(|ty| Suffix { signed: None, ty }))
    }

    fn after_zero(&mut self) -> R<(u32, &'a str)> {
        if self.opt(|r| r.token("x|X"))?.is_some() {
            return Ok((
                16,
                self.run(|c| c.is_ascii_hexdigit(), K::ExpectedHexNumeral)?,
            ));
        }
        let binary = self.opt(|r| {
            r.token("b|B")?;
            r.run(|c| matches!(c, '0' | '1'), K::ExpectedBinaryNumeral)
        })?;
        if let Some(digits) = binary {
            return Ok((2, digits));
        }
        if self.opt(Self::decimal)?.is_some() {
            return self.fail(self.pos, K::LeadingZeroNotAllowed);
        }
        Ok((10, "0"))
    }

    fn integer(&mut self) -> R<IntLiteral<'a>> {
        self.skip_whitespace();
        let body = self.pos;
        let negative = self.sign()?;
        let (radix, digits) = if self.opt(|r| r.token("0"))?.is_some() {
            self.after_zero()?
        } else {
            (10, self.decimal()?)
        };
        let suffix = self.integer_suffix()?;
        Ok(IntLiteral {
            negative,
            radix,
            digits,
            suffix,
            span: Span {
                start: body,
                end: self.pos,
            },
        })
    }

    fn check_integer(&mut self, literal: IntLiteral<'a>) -> R<(Scalar, bool)> {
        let ty = literal.suffix.map_or(IntegerType::Int, |suffix| suffix.ty);
        match integer_value(literal, ty) {
            Ok(value) => Ok((Scalar::integer(ty, value), value != 0)),
            Err(kind) => self.fail(self.pos, kind),
        }
    }

    fn quoted(&mut self) -> R<StrText<'a>> {
        if self.opt(|r| r.token("\""))?.is_some() {
            return self.string_body("\"", "'");
        }
        self.token("'")?;
        self.string_body("'", "\"")
    }

    fn string_body(&mut self, quote: &'static str, other: &'static str) -> R<StrText<'a>> {
        let body = self.pos;
        let mut after_high = false;
        loop {
            if self.opt(Self::string_text)?.is_some() {
                continue;
            }
            let escaped = self.opt(|r| {
                r.token("\\")?;
                r.escape(&mut after_high)
            })?;
            if escaped.is_some() || self.opt(|r| r.token(other))?.is_some() {
                continue;
            }
            break;
        }
        let text = self.input.get(body..self.pos).unwrap_or_default();
        self.token(quote)?;
        Ok(StrText::Quoted(text))
    }

    fn string_text(&mut self) -> R<()> {
        let rest = self.rest();
        let length = rest.find(['"', '\'', '\\']).unwrap_or(rest.len());
        if length == 0 {
            return self.fail(self.pos, K::InvalidStringContents);
        }
        self.pos += length;
        Ok(())
    }

    fn escape(&mut self, after_high: &mut bool) -> R<()> {
        let letter = self.first_token(&ESCAPE_LETTERS)?;
        if simple_escape(letter).is_some() {
            return Ok(());
        }
        let Some(length) = hex_length(letter) else {
            return self.named_escape();
        };
        let Some(code) = hex_digits(self.rest(), length) else {
            return self.fail(self.pos, K::ExpectedHexEscape(length as u8));
        };
        self.pos += length;
        let unpaired = if is_high_surrogate(code) {
            !low_surrogate_follows(self.rest())
        } else {
            is_low_surrogate(code) && !*after_high
        };
        *after_high = is_high_surrogate(code);
        if code > 0x10_ffff || unpaired {
            return self.fail(self.pos, K::InvalidCodepoint(code));
        }
        Ok(())
    }

    fn named_escape(&mut self) -> R<()> {
        self.token("{")?;
        let rest = self.rest();
        let length = rest.find(|c| !is_name_char(c)).unwrap_or(rest.len());
        let name = rest.get(..length).unwrap_or_default();
        if name.is_empty() {
            return self.fail(self.pos, K::InvalidCharacterName);
        }
        self.pos += length;
        self.token("}")?;
        if char_by_name(name).is_none() {
            return self.fail(self.pos, K::InvalidCharacterName);
        }
        Ok(())
    }

    fn compound(&mut self, depth: u32) -> R<(B::Value, Shape<'a>)> {
        self.skip_whitespace();
        too_deep(depth, self.pos)?;
        self.token("{")?;
        let mut compound = self.build.compound();
        self.items(|r| {
            let key = r.key()?;
            r.token(":")?;
            let (value, _) = r.value(depth + 1)?;
            if key.is_empty() {
                return r.fail(r.pos, K::EmptyKey);
            }
            r.build.insert(&mut compound, key, value);
            Ok(())
        })?;
        self.token("}")?;
        Ok((self.build.finish_compound(compound), Shape::Compound))
    }

    fn key(&mut self) -> R<StrText<'a>> {
        if let Some(text) = self.opt(Self::quoted)? {
            return Ok(text);
        }
        self.word().map(StrText::Word)
    }

    fn list(&mut self, depth: u32) -> R<(B::Value, Shape<'a>)> {
        self.skip_whitespace();
        let open = self.pos;
        self.token("[")?;
        let kind = self.opt(|r| {
            let kind = match r.first_token(&["B", "L", "I"])? {
                'B' => ArrayKind::Byte,
                'L' => ArrayKind::Long,
                _ => ArrayKind::Int,
            };
            r.token(";")?;
            Ok(kind)
        })?;
        if let Some(kind) = kind {
            return self.array(kind);
        }
        too_deep(depth, open)?;
        let mut list = self.build.list();
        self.items(|r| {
            let (value, _) = r.value(depth + 1)?;
            r.build.push(&mut list, value);
            Ok(())
        })?;
        self.token("]")?;
        Ok((self.build.finish_list(list), Shape::Other))
    }

    fn array(&mut self, kind: ArrayKind) -> R<(B::Value, Shape<'a>)> {
        let mut array = self.build.array(kind);
        let mut invalid = None;
        self.items(|r| {
            let literal = r.integer()?;
            if invalid.is_none() {
                match array_element(literal, kind) {
                    Ok(value) => r.build.push_element(&mut array, value),
                    Err(kind) => invalid = Some(kind),
                }
            }
            Ok(())
        })?;
        self.token("]")?;
        if let Some(kind) = invalid {
            return self.fail(self.pos, kind);
        }
        Ok((self.build.finish_array(array), Shape::Other))
    }

    fn word_value(&mut self, depth: u32) -> R<(B::Value, Shape<'a>)> {
        self.skip_whitespace();
        let start = self.pos;
        let word = self.word()?;
        let name = Span {
            start,
            end: self.pos,
        };
        let call = self.opt(|r| r.arguments(depth))?;
        if word.starts_with(is_number_start) {
            return self.fail(self.pos, K::InvalidUnquotedStart);
        }
        if let Some((arguments, first)) = call {
            return self.operation(word, name, arguments, first);
        }
        let truth = if word.eq_ignore_ascii_case("true") {
            Some(true)
        } else if word.eq_ignore_ascii_case("false") {
            Some(false)
        } else {
            None
        };
        Ok(match truth {
            Some(truth) => (
                self.build.scalar(Scalar::Byte(i8::from(truth))),
                Shape::Number { nonzero: truth },
            ),
            None => {
                let text = StrText::Word(word);
                (self.build.string(text), Shape::Str(text))
            }
        })
    }

    fn arguments(&mut self, depth: u32) -> R<(usize, Option<Shape<'a>>)> {
        self.skip_whitespace();
        let open = self.pos;
        self.token("(")?;
        too_deep(depth, open)?;
        let mut count = 0;
        let mut first = None;
        self.items(|r| {
            let (_, shape) = r.value(depth + 1)?;
            first.get_or_insert(shape);
            count += 1;
            Ok(())
        })?;
        self.token(")")?;
        Ok((count, first))
    }

    fn operation(
        &mut self,
        word: &str,
        name: Span,
        arguments: usize,
        first: Option<Shape<'a>>,
    ) -> R<(B::Value, Shape<'a>)> {
        match (word, arguments, first) {
            ("bool", 1, Some(Shape::Number { nonzero })) => Ok((
                self.build.scalar(Scalar::Byte(i8::from(nonzero))),
                Shape::Number { nonzero },
            )),
            ("bool", 1, _) => self.fail(self.pos, K::ExpectedNumberOrBoolean),
            ("uuid", 1, Some(Shape::Str(text))) => match canonical_uuid(text) {
                Some(uuid) => {
                    let mut array = self.build.array(ArrayKind::Int);
                    for group in uuid {
                        self.build.push_element(&mut array, i64::from(group));
                    }
                    Ok((self.build.finish_array(array), Shape::Other))
                }
                None => self.fail(self.pos, K::ExpectedStringUuid),
            },
            ("uuid", 1, _) => self.fail(self.pos, K::ExpectedStringUuid),
            _ => self.fail(self.pos, K::NoSuchOperation { name, arguments }),
        }
    }
}

fn too_deep(depth: u32, position: usize) -> R<()> {
    if depth > MAX_DEPTH {
        return Err(Fail::Abort(SnbtError {
            kind: K::TooDeep,
            position,
        }));
    }
    Ok(())
}

/// Whether the text starts with an escape of a low surrogate, without recording anything.
fn low_surrogate_follows(rest: &str) -> bool {
    let Some(rest) = rest.strip_prefix('\\') else {
        return false;
    };
    let mut rest = rest.trim_start_matches(is_whitespace).chars();
    let Some(length) = rest.next().and_then(hex_length) else {
        return false;
    };
    hex_digits(rest.as_str(), length).is_some_and(is_low_surrogate)
}

/// The four groups of 32 bits of a UUID in the canonical form (SNBT-DEV-002).
fn canonical_uuid(text: StrText<'_>) -> Option<[i32; 4]> {
    const DASHES: [usize; 4] = [8, 13, 18, 23];
    let mut value: u128 = 0;
    let mut length = 0;
    for (index, c) in text.chars().enumerate() {
        length += 1;
        if DASHES.contains(&index) {
            if c != '-' {
                return None;
            }
            continue;
        }
        value = (value << 4) | u128::from(c.to_digit(16)?);
    }
    if length != 36 {
        return None;
    }
    let group = |shift: u32| (value >> shift) as u32 as i32;
    Some([group(96), group(64), group(32), group(0)])
}

/// SNBT-VAL-003: the value of an integer literal as `ty`, sign-extended to 64 bits.
fn integer_value(literal: IntLiteral<'_>, ty: IntegerType) -> Result<i64, K> {
    let signed = literal
        .suffix
        .and_then(|suffix| suffix.signed)
        .unwrap_or(literal.radix == 10);
    if !signed && literal.negative {
        return Err(K::ExpectedNonNegativeNumber);
    }
    let failure = K::NumberParseFailure {
        number: literal.span,
        ty,
    };
    let mut magnitude: u128 = 0;
    for digit in literal
        .digits
        .chars()
        .filter_map(|c| c.to_digit(literal.radix))
    {
        magnitude = magnitude
            .checked_mul(u128::from(literal.radix))
            .and_then(|m| m.checked_add(u128::from(digit)))
            .filter(|&m| m <= u128::from(u64::MAX))
            .ok_or(failure)?;
    }
    let bits = ty.bits();
    let limit: u128 = match (signed, literal.negative) {
        (true, true) => 1 << (bits - 1),
        (true, false) => (1 << (bits - 1)) - 1,
        (false, _) => (1 << bits) - 1,
    };
    if magnitude > limit {
        return Err(failure);
    }
    let raw = if literal.negative {
        (magnitude as i128).wrapping_neg() as i64
    } else {
        magnitude as u64 as i64
    };
    let shift = 64 - bits;
    Ok((raw << shift) >> shift)
}

/// SNBT-VAL-005: the type of an element is its own, or the type of the array.
fn array_element(literal: IntLiteral<'_>, kind: ArrayKind) -> Result<i64, K> {
    let ty = literal.suffix.map_or(kind.element(), |suffix| suffix.ty);
    let allowed = match kind {
        ArrayKind::Byte => matches!(ty, IntegerType::Byte),
        ArrayKind::Int => !matches!(ty, IntegerType::Long),
        ArrayKind::Long => true,
    };
    if !allowed {
        return Err(K::InvalidArrayElementType);
    }
    integer_value(literal, ty)
}

const FLOAT_TEXT: usize = 128;

/// SNBT-VAL-004: the value rounded once from the decimal text, or `None` when it is infinite.
/// The value and whether it is not zero.
fn float_value(negative: bool, parts: &FloatParts<'_>) -> Option<(Scalar, bool)> {
    let mut stack = [0u8; FLOAT_TEXT];
    let mut heap = Vec::new();
    let pieces = [
        if negative { "-" } else { "" },
        parts.whole.unwrap_or("0"),
        ".",
        parts.fraction.unwrap_or("0"),
        if parts.exponent.is_some() { "e" } else { "" },
        match parts.exponent {
            Some((true, _)) => "-",
            _ => "",
        },
        parts.exponent.map_or("", |(_, digits)| digits),
    ];
    let length: usize = pieces.iter().map(|piece| piece.len()).sum();
    let bytes = pieces
        .iter()
        .flat_map(|piece| piece.bytes())
        .filter(|&b| b != b'_');
    // chisle: a number text longer than FLOAT_TEXT bytes allocates once; a decimal reader over
    // the pieces lifts it.
    let text: &[u8] = if length <= FLOAT_TEXT {
        let mut used = 0;
        for (slot, byte) in stack.iter_mut().zip(bytes) {
            *slot = byte;
            used += 1;
        }
        stack.get(..used)?
    } else {
        heap.extend(bytes);
        &heap
    };
    let text = core::str::from_utf8(text).ok()?;
    Some(if parts.float {
        let value = text.parse::<f32>().ok().filter(|v| v.is_finite())? + 0.0;
        (Scalar::Float(value), value != 0.0)
    } else {
        let value = text.parse::<f64>().ok().filter(|v| v.is_finite())? + 0.0;
        (Scalar::Double(value), value != 0.0)
    })
}
