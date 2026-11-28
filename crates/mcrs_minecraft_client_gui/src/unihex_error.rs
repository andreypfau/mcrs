use core::error::Error;
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnihexError {
    Inflate {
        reason: String,
    },
    NotText {
        file: String,
    },
    NoColon {
        file: String,
        line: usize,
    },
    BadCodePoint {
        file: String,
        line: usize,
    },
    DigitCount {
        file: String,
        line: usize,
        digits: usize,
    },
    BadDigit {
        file: String,
        line: usize,
    },
}

impl fmt::Display for UnihexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inflate { reason } => write!(formatter, "{reason}"),
            Self::NotText { file } => write!(formatter, "{file}: the file is not UTF-8"),
            Self::NoColon { file, line } => write!(formatter, "{file}: line {line} has no colon"),
            Self::BadCodePoint { file, line } => {
                write!(formatter, "{file}: line {line} has a bad code point")
            }
            Self::DigitCount { file, line, digits } => {
                write!(formatter, "{file}: line {line} has {digits} digits of data")
            }
            Self::BadDigit { file, line } => {
                write!(formatter, "{file}: line {line} has a bad digit in its data")
            }
        }
    }
}

impl Error for UnihexError {}
