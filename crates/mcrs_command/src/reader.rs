use crate::error::{LibraryError, Span, StringSpan, SyntaxError};

pub const SEPARATOR: char = ' ';

const fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | '\u{1c}'..='\u{20}'
            | '\u{1680}'
            | '\u{2000}'..='\u{2006}'
            | '\u{2008}'..='\u{200a}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

const fn is_number_char(c: char) -> bool {
    matches!(c, '0'..='9' | '.' | '-')
}

const fn is_unquoted_char(c: char) -> bool {
    matches!(c, '0'..='9' | 'A'..='Z' | 'a'..='z' | '_' | '-' | '.' | '+')
}

const fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'')
}

/// A cursor over a borrowed command line. Positions are byte offsets; no operation stops
/// inside a character.
#[derive(Clone, Copy, Debug)]
pub struct Reader<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> Reader<'a> {
    pub const fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    pub const fn at(input: &'a str, cursor: usize) -> Self {
        Self { input, cursor }
    }

    pub const fn input(&self) -> &'a str {
        self.input
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.input.len());
    }

    pub fn remaining(&self) -> &'a str {
        self.input.get(self.cursor..).unwrap_or_default()
    }

    pub const fn at_end(&self) -> bool {
        self.cursor >= self.input.len()
    }

    pub fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    pub fn peek_is(&self, c: char) -> bool {
        self.peek() == Some(c)
    }

    pub fn peek_nth(&self, n: usize) -> Option<char> {
        self.remaining().chars().nth(n)
    }

    pub fn skip(&mut self) {
        self.read();
    }

    pub fn read(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.cursor += c.len_utf8();
        Some(c)
    }

    pub fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.cursor)
    }

    pub fn read_while(&mut self, accept: impl Fn(char) -> bool) -> Span {
        let start = self.cursor;
        while let Some(c) = self.peek().filter(|&c| accept(c)) {
            self.cursor += c.len_utf8();
        }
        self.span_from(start)
    }

    pub fn skip_whitespace(&mut self) {
        self.read_while(is_whitespace);
    }

    pub fn expect<E>(&mut self, c: char) -> Result<(), SyntaxError<E>> {
        if self.peek_is(c) {
            self.skip();
            Ok(())
        } else {
            Err(SyntaxError::library(
                LibraryError::ExpectedSymbol(c),
                self.cursor,
            ))
        }
    }

    pub fn read_unquoted(&mut self) -> Span {
        self.read_while(is_unquoted_char)
    }

    pub fn read_int<E>(&mut self) -> Result<i32, SyntaxError<E>> {
        self.read_number(LibraryError::ExpectedInt, LibraryError::InvalidInt, |_| {
            true
        })
    }

    pub fn read_long<E>(&mut self) -> Result<i64, SyntaxError<E>> {
        self.read_number(
            LibraryError::ExpectedLong,
            LibraryError::InvalidLong,
            |_| true,
        )
    }

    pub fn read_double<E>(&mut self) -> Result<f64, SyntaxError<E>> {
        self.read_number(
            LibraryError::ExpectedDouble,
            LibraryError::InvalidDouble,
            f64::is_finite,
        )
    }

    pub fn read_float<E>(&mut self) -> Result<f32, SyntaxError<E>> {
        self.read_number(
            LibraryError::ExpectedFloat,
            LibraryError::InvalidFloat,
            f32::is_finite,
        )
    }

    /// The token holds only number characters, so `FromStr` sees no sign `+`, no exponent and
    /// no `inf` or `nan`.
    fn read_number<T: core::str::FromStr + Copy, E>(
        &mut self,
        expected: LibraryError,
        invalid: fn(Span) -> LibraryError,
        accept: fn(T) -> bool,
    ) -> Result<T, SyntaxError<E>> {
        let token = self.read_while(is_number_char);
        if token.is_empty() {
            return Err(SyntaxError::library(expected, self.cursor));
        }
        match token
            .text(self.input)
            .parse()
            .ok()
            .filter(|&value| accept(value))
        {
            Some(value) => Ok(value),
            None => {
                self.cursor = token.start;
                Err(SyntaxError::library(invalid(token), token.start))
            }
        }
    }

    /// Reads the body of a quoted string; the cursor is after the opening quote.
    fn read_quoted_body<E>(&mut self, terminator: char) -> Result<StringSpan, SyntaxError<E>> {
        let start = self.cursor;
        loop {
            let Some(c) = self.read() else {
                return Err(SyntaxError::library(
                    LibraryError::ExpectedEndOfQuote,
                    self.cursor,
                ));
            };
            if c == '\\' {
                match self.peek() {
                    Some(next) if next == terminator || next == '\\' => self.skip(),
                    Some(next) => {
                        return Err(SyntaxError::library(
                            LibraryError::InvalidEscape(next),
                            self.cursor,
                        ));
                    }
                    None => {
                        return Err(SyntaxError::library(
                            LibraryError::ExpectedEndOfQuote,
                            self.cursor,
                        ));
                    }
                }
            } else if c == terminator {
                return Ok(StringSpan {
                    span: Span::new(start, self.cursor - c.len_utf8()),
                    quoted: true,
                });
            }
        }
    }

    pub fn read_string<E>(&mut self) -> Result<StringSpan, SyntaxError<E>> {
        match self.peek() {
            Some(quote) if is_quote(quote) => {
                self.skip();
                self.read_quoted_body(quote)
            }
            _ => Ok(StringSpan {
                span: self.read_unquoted(),
                quoted: false,
            }),
        }
    }

    pub fn read_bool<E>(&mut self) -> Result<bool, SyntaxError<E>> {
        let start = self.cursor;
        let string = self.read_string()?;
        let value = StringValue::new(self.input, string);
        if value.is_empty() {
            return Err(SyntaxError::library(
                LibraryError::ExpectedBool,
                self.cursor,
            ));
        }
        if value.eq_str("true") {
            Ok(true)
        } else if value.eq_str("false") {
            Ok(false)
        } else {
            self.cursor = start;
            Err(SyntaxError::library(
                LibraryError::InvalidBool(string),
                start,
            ))
        }
    }
}

/// The value of a string the reader read, borrowed from the command line.
#[derive(Clone, Copy, Debug)]
pub struct StringValue<'a> {
    raw: &'a str,
    quoted: bool,
}

impl<'a> StringValue<'a> {
    pub fn new(input: &'a str, string: StringSpan) -> Self {
        Self {
            raw: string.span.text(input),
            quoted: string.quoted,
        }
    }

    /// The text without escapes, borrowed when it has none.
    pub fn as_unescaped(&self) -> Option<&'a str> {
        (!self.quoted || !self.raw.contains('\\')).then_some(self.raw)
    }

    pub fn chars(&self) -> impl Iterator<Item = char> + 'a {
        let mut escaped = false;
        let quoted = self.quoted;
        self.raw.chars().filter(move |&c| {
            if quoted && c == '\\' && !escaped {
                escaped = true;
                return false;
            }
            escaped = false;
            true
        })
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    pub fn eq_str(&self, other: &str) -> bool {
        self.chars().eq(other.chars())
    }

    pub fn len_utf16(&self) -> usize {
        self.chars().map(char::len_utf16).sum()
    }
}
