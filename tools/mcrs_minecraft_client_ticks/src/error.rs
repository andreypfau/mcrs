/// A line of a recording that does not parse.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("line {line}: {kind}")]
pub struct ParseError {
    /// The line number, from 1.
    pub line: usize,
    pub kind: ParseErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseErrorKind {
    /// The first line is not the format line of a supported format version.
    #[error("the recording does not start with `{}`", crate::FORMAT_LINE)]
    Format,
    /// A header line names no known header field.
    #[error("unknown header field `{0}`")]
    UnknownHeader(String),
    /// A header field that every recording has is not there.
    #[error("the header has no `{0}` line")]
    MissingHeader(&'static str),
    /// An event line has a direction and a kind that no event has.
    #[error("unknown event `{0}`")]
    UnknownEvent(String),
    /// A line ends before all of its fields.
    #[error("the line ends before the field `{0}`")]
    MissingField(&'static str),
    /// A field does not parse as its type.
    #[error("the field `{field}` has the bad value `{value}`")]
    BadField { field: &'static str, value: String },
    /// A line has more fields than its kind.
    #[error("the line has the extra field `{0}`")]
    ExtraField(String),
}
