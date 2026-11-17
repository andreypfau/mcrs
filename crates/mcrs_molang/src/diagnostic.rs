use alloc::string::String;
use core::fmt;
use core::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    #[must_use]
    pub fn new(range: Range<usize>) -> Self {
        Self {
            start: u32::try_from(range.start).unwrap_or(u32::MAX),
            end: u32::try_from(range.end).unwrap_or(u32::MAX),
        }
    }

    #[must_use]
    pub fn to(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    Syntax,
    Type,
    UnknownFunction,
    UnknownQuery,
    UnknownResource,
    Arity,
    ReadOnly,
    Reserved,
    TempStruct,
    Misplaced,
    StaticType,
    Unsupported,
}

impl ErrorKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Type => "type",
            Self::UnknownFunction => "unknown_function",
            Self::UnknownQuery => "unknown_query",
            Self::UnknownResource => "unknown_resource",
            Self::Arity => "arity",
            Self::ReadOnly => "read_only",
            Self::Reserved => "reserved",
            Self::TempStruct => "temp_struct",
            Self::Misplaced => "misplaced",
            Self::StaticType => "static_type",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: ErrorKind,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    #[must_use]
    pub fn new(kind: ErrorKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {}..{}: {}",
            self.kind.name(),
            self.span.start,
            self.span.end,
            self.message
        )
    }
}

impl core::error::Error for Diagnostic {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WarningKind {
    UnknownQuery,
    QueryOutOfContext,
    OldMolangVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub kind: WarningKind,
    pub span: Span,
    pub name: String,
}
