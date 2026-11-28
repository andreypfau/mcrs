use core::fmt;
use mcrs_command::Span;
use mcrs_command::StringSpan;
use mcrs_minecraft_command::{
    ErrorText, ResourceError, identifier_string, identifier_text, snbt_text, string_text,
};
use mcrs_minecraft_snbt::SnbtError;
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum BlockError {
    BlockIdInvalid(Span),
    BlockTagUnknown(Span),
    TagDisallowed,
    PropertyUnknown {
        block: Span,
        name: StringSpan,
    },
    PropertyDuplicate {
        name: StringSpan,
        subject: BlockSubject,
    },
    PropertyNoValue {
        name: StringSpan,
        subject: BlockSubject,
    },
    PropertyInvalid {
        block: Span,
        value: StringSpan,
        name: StringSpan,
    },
    PropertyUnclosed,
    Nbt(SnbtError),
    Resource(ResourceError),
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BlockError::BlockIdInvalid(_) => "unknown block",
            BlockError::BlockTagUnknown(_) => "unknown block tag",
            BlockError::TagDisallowed => "a block state names no tag",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockSubject {
    Block(Span),
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
            TagDisallowed => Text::translate("argument.block.tag.disallowed", vec![]),
            Nbt(error) => snbt_text(input, error),
            Resource(error) => error.text(input),
        }
    }
}
