use core::fmt;

use mcrs_minecraft_snbt::SnbtError;

/// A text that is not an NBT path, with the byte offset of the fault in the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbtPathError {
    /// A node is expected: the text is empty, ends with `.`, or has no node after a `.`.
    ExpectedNode { position: usize },
    /// A node ends and the next character is neither `.` nor `[`.
    ExpectedSeparator { position: usize },
    /// A quoted name has no closing quote before the end of the text.
    UnterminatedQuote { position: usize },
    /// A quoted name has no character.
    EmptyName { position: usize },
    /// A `\` in a quoted name is followed by a character other than the quote or `\`.
    InvalidEscape { position: usize },
    /// A `[` is followed by neither `]`, an integer nor a compound.
    ExpectedIndex { position: usize },
    /// An index does not fit in an int.
    IndexOutOfRange { position: usize },
    /// An index node has no `]` at its end.
    ExpectedClosingBracket { position: usize },
    /// A compound pattern is not valid SNBT; the SNBT error holds the offset.
    Pattern(SnbtError),
}

impl NbtPathError {
    /// The byte offset of the fault in the text.
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
