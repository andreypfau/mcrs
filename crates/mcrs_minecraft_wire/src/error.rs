use std::borrow::Cow;
use std::{fmt, io, str};

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Error(Box<Repr>);

struct Repr {
    kind: ErrorKind,
    context: Vec<Cow<'static, str>>,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ErrorKind {
    Io(io::Error),
    UnexpectedEnd,
    TrailingBytes(usize),
    VarIntTooLong,
    NegativeLength(i32),
    TooLong { len: usize, max: usize },
    InvalidUtf8(str::Utf8Error),
    DuplicateKey,
    TooDeep { max: u32 },
    UnknownTag { ty: &'static str, tag: i32 },
    Invalid(Cow<'static, str>),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
    pub fn invalid(message: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::Invalid(message.into()).into()
    }

    pub fn other(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        ErrorKind::Other(error.into()).into()
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.0.kind
    }

    pub fn context(&self) -> impl Iterator<Item = &str> {
        self.0.context.iter().rev().map(|c| &**c)
    }

    #[must_use]
    pub fn with_context(mut self, context: impl Into<Cow<'static, str>>) -> Self {
        self.0.context.push(context.into());
        self
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error(Box::new(Repr {
            kind,
            context: Vec::new(),
        }))
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        ErrorKind::Io(error).into()
    }
}

impl From<str::Utf8Error> for Error {
    fn from(error: str::Utf8Error) -> Self {
        ErrorKind::InvalidUtf8(error).into()
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Io(e) => write!(f, "{e}"),
            ErrorKind::UnexpectedEnd => f.write_str("the input ends before the value"),
            ErrorKind::TrailingBytes(len) => write!(f, "{len} bytes left over"),
            ErrorKind::VarIntTooLong => f.write_str("a variable-length integer is too long"),
            ErrorKind::NegativeLength(len) => write!(f, "negative length {len}"),
            ErrorKind::TooLong { len, max } => write!(f, "length {len} exceeds the maximum {max}"),
            ErrorKind::InvalidUtf8(e) => write!(f, "{e}"),
            ErrorKind::DuplicateKey => f.write_str("duplicate map key"),
            ErrorKind::TooDeep { max } => write!(f, "value nested deeper than {max} levels"),
            ErrorKind::UnknownTag { ty, tag } => write!(f, "unknown discriminant {tag} in `{ty}`"),
            ErrorKind::Invalid(message) => f.write_str(message),
            ErrorKind::Other(e) => write!(f, "{e}"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for context in self.context() {
            write!(f, "{context}: ")?;
        }
        self.0.kind.fmt(f)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.0.kind)
            .field("context", &self.0.context)
            .finish()
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0.kind {
            ErrorKind::Io(e) => Some(e),
            ErrorKind::InvalidUtf8(e) => Some(e),
            ErrorKind::Other(e) => Some(&**e),
            _ => None,
        }
    }
}

pub trait Context<T> {
    fn context(self, context: impl Into<Cow<'static, str>>) -> Result<T>;

    fn with_context<C: Into<Cow<'static, str>>>(self, context: impl FnOnce() -> C) -> Result<T>;
}

impl<T> Context<T> for Result<T> {
    fn context(self, context: impl Into<Cow<'static, str>>) -> Result<T> {
        self.map_err(|e| e.with_context(context))
    }

    fn with_context<C: Into<Cow<'static, str>>>(self, context: impl FnOnce() -> C) -> Result<T> {
        self.map_err(|e| e.with_context(context()))
    }
}

impl<T> Context<T> for Option<T> {
    fn context(self, context: impl Into<Cow<'static, str>>) -> Result<T> {
        self.ok_or_else(|| Error::invalid(context))
    }

    fn with_context<C: Into<Cow<'static, str>>>(self, context: impl FnOnce() -> C) -> Result<T> {
        self.ok_or_else(|| Error::invalid(context()))
    }
}

#[macro_export]
macro_rules! bail {
    ($($arg:tt)+) => {
        return ::core::result::Result::Err($crate::Error::invalid(::std::format!($($arg)+)))
    };
}

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            $crate::bail!($($arg)+);
        }
    };
}
