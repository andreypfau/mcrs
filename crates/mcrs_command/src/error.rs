use core::fmt;

use mcrs_text_span::Span;

use crate::reader::StringSpan;

/// The error kinds of the command library. A kind that names text from the command line holds
/// its span, never a copy.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
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

fn invalid(f: &mut fmt::Formatter<'_>, span: Span, kind: &str) -> fmt::Result {
    write!(
        f,
        "the text at bytes {}..{} is not a valid {kind}",
        span.start, span.end
    )
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::DoubleTooLow { value, min } => {
                write!(f, "the double {value} is below the minimum {min}")
            }
            LibraryError::DoubleTooHigh { value, max } => {
                write!(f, "the double {value} is above the maximum {max}")
            }
            LibraryError::FloatTooLow { value, min } => {
                write!(f, "the float {value} is below the minimum {min}")
            }
            LibraryError::FloatTooHigh { value, max } => {
                write!(f, "the float {value} is above the maximum {max}")
            }
            LibraryError::IntegerTooLow { value, min } => {
                write!(f, "the integer {value} is below the minimum {min}")
            }
            LibraryError::IntegerTooHigh { value, max } => {
                write!(f, "the integer {value} is above the maximum {max}")
            }
            LibraryError::LongTooLow { value, min } => {
                write!(f, "the long {value} is below the minimum {min}")
            }
            LibraryError::LongTooHigh { value, max } => {
                write!(f, "the long {value} is above the maximum {max}")
            }
            LibraryError::ExpectedEndOfQuote => f.write_str("the quoted string has no end"),
            LibraryError::InvalidEscape(c) => {
                write!(f, "the escape of {c:?} in a quoted string is not valid")
            }
            LibraryError::InvalidBool(string) => invalid(f, string.span, "bool"),
            LibraryError::ExpectedBool => f.write_str("expected a bool"),
            LibraryError::InvalidInt(span) => invalid(f, *span, "integer"),
            LibraryError::ExpectedInt => f.write_str("expected an integer"),
            LibraryError::InvalidLong(span) => invalid(f, *span, "long"),
            LibraryError::ExpectedLong => f.write_str("expected a long"),
            LibraryError::InvalidDouble(span) => invalid(f, *span, "double"),
            LibraryError::ExpectedDouble => f.write_str("expected a double"),
            LibraryError::InvalidFloat(span) => invalid(f, *span, "float"),
            LibraryError::ExpectedFloat => f.write_str("expected a float"),
            LibraryError::ExpectedSymbol(c) => write!(f, "expected {c:?}"),
            LibraryError::UnknownCommand => f.write_str("the command is not known"),
            LibraryError::UnknownArgument => f.write_str("the argument is not known"),
            LibraryError::ExpectedSeparator => f.write_str("expected a space"),
        }
    }
}

impl core::error::Error for LibraryError {}

/// The source of a syntax error: the library, or an argument type of the application.
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
    /// An error of the library at `position`.
    pub const fn library(kind: LibraryError, position: usize) -> Self {
        Self {
            kind: ErrorKind::Library(kind),
            position,
        }
    }

    /// An error of an argument type at `position`.
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

impl<E: fmt::Display> fmt::Display for SyntaxError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Library(kind) => write!(f, "{kind} at byte {}", self.position),
            ErrorKind::Argument(kind) => write!(f, "{kind} at byte {}", self.position),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> core::error::Error for SyntaxError<E> {}
