use mcrs_text_span::{Span, is_whitespace};

use crate::error::{LibraryError, SyntaxError};

/// The character between two nodes of a command line.
pub const SEPARATOR: char = ' ';

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
    /// A reader at the start of `input`.
    pub const fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    /// A reader at byte `cursor` of `input`.
    pub const fn at(input: &'a str, cursor: usize) -> Self {
        Self { input, cursor }
    }

    /// The whole command line.
    pub const fn input(&self) -> &'a str {
        self.input
    }

    /// The byte offset of the next character.
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor, clamped to the end of the input.
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.input.len());
    }

    /// The input from the cursor on.
    pub fn remaining(&self) -> &'a str {
        self.input.get(self.cursor..).unwrap_or_default()
    }

    /// Whether the cursor is at the end of the input.
    pub const fn at_end(&self) -> bool {
        self.cursor >= self.input.len()
    }

    /// The character at the cursor.
    pub fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    /// Whether the character at the cursor is `c`.
    pub fn peek_is(&self, c: char) -> bool {
        self.peek() == Some(c)
    }

    /// The character `n` characters after the cursor.
    pub fn peek_nth(&self, n: usize) -> Option<char> {
        self.remaining().chars().nth(n)
    }

    /// Moves the cursor over one character.
    pub fn skip(&mut self) {
        self.read();
    }

    /// The character at the cursor, which the cursor moves over.
    pub fn read(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.cursor += c.len_utf8();
        Some(c)
    }

    /// The span from `start` to the cursor.
    pub fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.cursor)
    }

    /// Moves the cursor over the characters that `accept` and returns their span.
    pub fn read_while(&mut self, accept: impl Fn(char) -> bool) -> Span {
        let start = self.cursor;
        while let Some(c) = self.peek().filter(|&c| accept(c)) {
            self.cursor += c.len_utf8();
        }
        self.span_from(start)
    }

    /// Moves the cursor over whitespace.
    pub fn skip_whitespace(&mut self) {
        self.read_while(is_whitespace);
    }

    /// Moves the cursor over `c`, or fails when another character is at the cursor.
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

    /// Reads the characters an unquoted string allows.
    pub fn read_unquoted(&mut self) -> Span {
        self.read_while(is_unquoted_char)
    }

    /// Reads an `i32`; on an invalid number the cursor stays at its start.
    pub fn read_int<E>(&mut self) -> Result<i32, SyntaxError<E>> {
        self.read_number(LibraryError::ExpectedInt, LibraryError::InvalidInt, |_| {
            true
        })
    }

    /// Reads an `i64`; on an invalid number the cursor stays at its start.
    pub fn read_long<E>(&mut self) -> Result<i64, SyntaxError<E>> {
        self.read_number(
            LibraryError::ExpectedLong,
            LibraryError::InvalidLong,
            |_| true,
        )
    }

    /// Reads a finite `f64`; on an invalid number the cursor stays at its start.
    pub fn read_double<E>(&mut self) -> Result<f64, SyntaxError<E>> {
        self.read_number(
            LibraryError::ExpectedDouble,
            LibraryError::InvalidDouble,
            f64::is_finite,
        )
    }

    /// Reads a finite `f32`; on an invalid number the cursor stays at its start.
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

    /// Reads a quoted string, or an unquoted one when no quote is at the cursor.
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

    /// Reads `true` or `false`; on another string the cursor stays at its start.
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
        if value == "true" {
            Ok(true)
        } else if value == "false" {
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

/// A string the reader read: the text between the quotes when `quoted`, with its escapes
/// still in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StringSpan {
    pub span: Span,
    pub quoted: bool,
}

/// The value of a string the reader read, borrowed from the command line.
#[derive(Clone, Copy, Debug)]
pub struct StringValue<'a> {
    raw: &'a str,
    quoted: bool,
}

impl<'a> StringValue<'a> {
    /// The value of `string`, a string read from `input`.
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

    /// The characters of the value, escapes removed.
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

    /// The length of the value in UTF-16 code units.
    pub fn len_utf16(&self) -> usize {
        self.chars().map(char::len_utf16).sum()
    }
}

impl PartialEq<str> for StringValue<'_> {
    fn eq(&self, other: &str) -> bool {
        self.chars().eq(other.chars())
    }
}

impl PartialEq<&str> for StringValue<'_> {
    fn eq(&self, other: &&str) -> bool {
        *self == **other
    }
}
