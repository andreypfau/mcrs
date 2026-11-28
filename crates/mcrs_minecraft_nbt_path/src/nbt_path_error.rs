use core::fmt;

use mcrs_minecraft_snbt::SnbtError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbtPathError {
    ExpectedNode { position: usize },
    ExpectedSeparator { position: usize },
    UnterminatedQuote { position: usize },
    EmptyName { position: usize },
    InvalidEscape { position: usize },
    ExpectedIndex { position: usize },
    IndexOutOfRange { position: usize },
    ExpectedClosingBracket { position: usize },
    Pattern(SnbtError),
}

impl NbtPathError {
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            NbtPathError::ExpectedNode { position }
            | NbtPathError::ExpectedSeparator { position }
            | NbtPathError::UnterminatedQuote { position }
            | NbtPathError::EmptyName { position }
            | NbtPathError::InvalidEscape { position }
            | NbtPathError::ExpectedIndex { position }
            | NbtPathError::IndexOutOfRange { position }
            | NbtPathError::ExpectedClosingBracket { position } => position,
            NbtPathError::Pattern(error) => error.position,
        }
    }
}

impl fmt::Display for NbtPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            NbtPathError::ExpectedNode { .. } => "expected a path node",
            NbtPathError::ExpectedSeparator { .. } => "expected `.` or `[` after a path node",
            NbtPathError::UnterminatedQuote { .. } => "expected the closing quote of a name",
            NbtPathError::EmptyName { .. } => "a quoted name is empty",
            NbtPathError::InvalidEscape { .. } => "invalid escape in a quoted name",
            NbtPathError::ExpectedIndex { .. } => "expected an index, a compound or `]`",
            NbtPathError::IndexOutOfRange { .. } => "the index does not fit in an int",
            NbtPathError::ExpectedClosingBracket { .. } => "expected `]`",
            NbtPathError::Pattern(error) => return write!(f, "invalid path pattern: {error}"),
        };
        write!(f, "{message} at {}", self.position())
    }
}

impl core::error::Error for NbtPathError {}
