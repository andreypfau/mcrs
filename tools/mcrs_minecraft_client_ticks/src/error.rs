/// A line of a recording that does not parse.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {message}")]
pub struct ParseError {
    /// The line number, from 1.
    pub line: usize,
    pub message: String,
}
