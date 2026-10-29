use mcrs_minecraft_nbt::MAX_DEPTH;
use mcrs_text_span::{Span, is_whitespace};

use crate::build::{ArrayKind, Build, Scalar};
use crate::error::{SnbtError, SnbtErrorKind as K};
use crate::number::array_element;
use crate::text::{
    ESCAPE_LETTERS, StrText, char_by_name, hex_digits, hex_length, is_high_surrogate,
    is_low_surrogate, is_name_char, is_number_start, is_unquoted, simple_escape,
};

pub(crate) enum Fail {
    Soft,
    Abort(SnbtError),
}

pub(crate) type R<T> = Result<T, Fail>;

/// What an operation and the compound read need to know about a value, whatever the builder.
#[derive(Clone, Copy)]
pub(crate) enum Shape<'a> {
    Number { nonzero: bool },
    Str(StrText<'a>),
    Compound,
    Other,
}

pub(crate) struct Reader<'a, B> {
    pub(crate) input: &'a str,
    pub(crate) pos: usize,
    best: Option<SnbtError>,
    pub(crate) build: B,
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

    pub(crate) fn fail<T>(&mut self, position: usize, kind: K) -> R<T> {
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
    pub(crate) fn token(&mut self, set: &'static str) -> R<char> {
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

    pub(crate) fn opt<T>(&mut self, read: impl FnOnce(&mut Self) -> R<T>) -> R<Option<T>> {
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

    pub(crate) fn first_token(&mut self, sets: &[&'static str]) -> R<char> {
        for set in sets {
            if let Some(c) = self.opt(|r| r.token(set))? {
                return Ok(c);
            }
        }
        Err(Fail::Soft)
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
