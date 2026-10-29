use core::fmt;
use mcrs_command::StringSpan;
use mcrs_minecraft_command::{
    ErrorText, ResourceError, identifier_string, identifier_text, snbt_text, string_text,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_snbt::SnbtError;
use mcrs_text_span::Span;

/// The errors of a block predicate.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum BlockError {
    /// The identifier at the span names no block.
    BlockIdInvalid(Span),
    /// The identifier at the span names no block tag.
    BlockTagUnknown(Span),
    /// The block at `block` has no property `name`.
    PropertyUnknown { block: Span, name: StringSpan },
    /// The property `name` appears twice in the list of `subject`.
    PropertyDuplicate {
        name: StringSpan,
        subject: BlockSubject,
    },
    /// The property `name` in the list of `subject` has no `=` and value.
    PropertyNoValue {
        name: StringSpan,
        subject: BlockSubject,
    },
    /// The property `name` of the block at `block` has no value `value`.
    PropertyInvalid {
        block: Span,
        value: StringSpan,
        name: StringSpan,
    },
    /// The property list has no closing `]`.
    PropertyUnclosed,
    /// An error of the SNBT read; its spans are positions in the command line.
    Nbt(SnbtError),
    /// An error of the identifier of the block or the tag.
    Resource(ResourceError),
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BlockError::BlockIdInvalid(_) => "unknown block",
            BlockError::BlockTagUnknown(_) => "unknown block tag",
            BlockError::PropertyUnknown { .. } => "unknown block property",
            BlockError::PropertyDuplicate { .. } => "duplicate block property",
            BlockError::PropertyNoValue { .. } => "block property without a value",
            BlockError::PropertyInvalid { .. } => "invalid block property value",
            BlockError::PropertyUnclosed => "unclosed block property list",
            BlockError::Nbt(_) => "invalid SNBT",
            BlockError::Resource(error) => return fmt::Display::fmt(error, f),
        })
    }
}

impl core::error::Error for BlockError {}

/// The block identifier or the tag that an error of a property list names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockSubject {
    /// The span of a block identifier.
    Block(Span),
    /// The span of a tag identifier, after its `#`.
    Tag(Span),
}

impl From<ResourceError> for BlockError {
    fn from(error: ResourceError) -> Self {
        BlockError::Resource(error)
    }
}

fn subject_text(input: &str, subject: BlockSubject) -> Text {
    match subject {
        BlockSubject::Block(span) => identifier_string(input, span),
        BlockSubject::Tag(span) => format!("#{}", identifier_string(input, span)),
    }
    .into_text()
}

impl ErrorText for BlockError {
    fn text(&self, input: &str) -> Text {
        use BlockError::*;
        match *self {
            BlockIdInvalid(id) => Text::translate(
                "argument.block.id.invalid",
                vec![identifier_text(input, id)],
            ),
            BlockTagUnknown(id) => Text::translate(
                "arguments.block.tag.unknown",
                vec![identifier_text(input, id)],
            ),
            PropertyUnknown { block, name } => Text::translate(
                "argument.block.property.unknown",
                vec![identifier_text(input, block), string_text(input, name)],
            ),
            PropertyDuplicate { name, subject } => Text::translate(
                "argument.block.property.duplicate",
                vec![string_text(input, name), subject_text(input, subject)],
            ),
            PropertyNoValue { name, subject } => Text::translate(
                "argument.block.property.novalue",
                vec![string_text(input, name), subject_text(input, subject)],
            ),
            PropertyInvalid { block, value, name } => Text::translate(
                "argument.block.property.invalid",
                vec![
                    identifier_text(input, block),
                    string_text(input, value),
                    string_text(input, name),
                ],
            ),
            PropertyUnclosed => Text::translate("argument.block.property.unclosed", vec![]),
            Nbt(error) => snbt_text(input, error),
            Resource(error) => error.text(input),
        }
    }
}
