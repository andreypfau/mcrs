/// A range of bytes of the command line, `start ≤ end`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub(crate) const fn contains(self, position: usize) -> bool {
        self.start <= position && position <= self.end
    }

    pub fn text(self, input: &str) -> &str {
        input.get(self.start..self.end).unwrap_or_default()
    }
}

/// A string the reader read: the text between the quotes when `quoted`, with its escapes
/// still in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StringSpan {
    pub span: Span,
    pub quoted: bool,
}

/// The error kinds of the command library. A kind that names text from the command line holds
/// its span, never a copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LibraryError {
    DoubleTooLow { value: f64, min: f64 },
    DoubleTooHigh { value: f64, max: f64 },
    FloatTooLow { value: f32, min: f32 },
    FloatTooHigh { value: f32, max: f32 },
    IntegerTooLow { value: i32, min: i32 },
    IntegerTooHigh { value: i32, max: i32 },
    LongTooLow { value: i64, min: i64 },
    LongTooHigh { value: i64, max: i64 },
    ExpectedEndOfQuote,
    InvalidEscape(char),
    InvalidBool(StringSpan),
    ExpectedBool,
    InvalidInt(Span),
    ExpectedInt,
    InvalidLong(Span),
    ExpectedLong,
    InvalidDouble(Span),
    ExpectedDouble,
    InvalidFloat(Span),
    ExpectedFloat,
    ExpectedSymbol(char),
    UnknownCommand,
    UnknownArgument,
    ExpectedSeparator,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ErrorKind<E> {
    Library(LibraryError),
    Argument(E),
}

/// A syntax error at a byte position of the command line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyntaxError<E> {
    pub kind: ErrorKind<E>,
    pub position: usize,
}

impl<E> SyntaxError<E> {
    pub const fn library(kind: LibraryError, position: usize) -> Self {
        Self {
            kind: ErrorKind::Library(kind),
            position,
        }
    }

    pub const fn argument(kind: E, position: usize) -> Self {
        Self {
            kind: ErrorKind::Argument(kind),
            position,
        }
    }

    /// The same error, with its argument error converted.
    pub fn map_into<F: From<E>>(self) -> SyntaxError<F> {
        SyntaxError {
            kind: match self.kind {
                ErrorKind::Library(kind) => ErrorKind::Library(kind),
                ErrorKind::Argument(kind) => ErrorKind::Argument(F::from(kind)),
            },
            position: self.position,
        }
    }
}

/// The position counted in UTF-16 code units, the unit of every position on the wire.
pub fn utf16_position(input: &str, position: usize) -> usize {
    input
        .get(..input.floor_char_boundary(position))
        .map_or(0, |start| start.encode_utf16().count())
}
