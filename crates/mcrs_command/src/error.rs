use mcrs_text_span::Span;

use crate::reader::StringSpan;

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
