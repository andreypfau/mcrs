use alloc::string::String;
use core::fmt;
use core::ops::Range;

/// A byte range in the expression text.
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

/// The kind of a load error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The text does not match the grammar.
    Syntax,
    /// The JSON value has a type that a Molang field does not accept.
    Type,
    /// A `math.` name that the language does not define.
    UnknownFunction,
    /// A query name that the host does not declare, where that is an error.
    UnknownQuery,
    /// A resource short name that the resource set does not have.
    UnknownResource,
    /// A call with a number of arguments that the definition does not allow.
    Arity,
    /// An assignment to a namespace that cannot be written.
    ReadOnly,
    /// A name with no namespace that is not a keyword.
    Reserved,
    /// A path of more than one name after `temp.`.
    TempStruct,
    /// `break` or `continue` outside a loop, or a resource outside a resource context.
    Misplaced,
    /// An operand of a type that the operator does not take, known at load.
    StaticType,
    /// A construct of a later milestone.
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

/// A load error.
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

/// The kind of a load warning. The expression still runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WarningKind {
    /// A query that the host does not declare; it reads as missing.
    UnknownQuery,
    /// A query whose declaration does not include the context of use; it reads as missing.
    QueryOutOfContext,
    /// A versioned value with a `version` below the modern rules.
    OldMolangVersion,
}

/// A load warning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub kind: WarningKind,
    pub span: Span,
    /// The name that the warning is about, for example the query name.
    pub name: String,
}
