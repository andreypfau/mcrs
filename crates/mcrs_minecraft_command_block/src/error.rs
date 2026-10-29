use mcrs_command::StringSpan;
use mcrs_minecraft_command::{
    ErrorText, ResourceError, identifier_string, identifier_text, snbt_text, string_text,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_snbt::SnbtError;
use mcrs_text_span::Span;

/// The errors of a block predicate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockError {
    BlockIdInvalid(Span),
    BlockTagUnknown(Span),
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
    /// An error of the SNBT read; its spans are positions in the command line.
    Nbt(SnbtError),
    Resource(ResourceError),
}

/// The block identifier or the tag that an error of a property list names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockSubject {
    pub span: Span,
    pub tag: bool,
}

impl BlockSubject {
    pub const fn block(span: Span) -> Self {
        Self { span, tag: false }
    }

    pub const fn tag(span: Span) -> Self {
        Self { span, tag: true }
    }
}

impl From<ResourceError> for BlockError {
    fn from(error: ResourceError) -> Self {
        BlockError::Resource(error)
    }
}

fn subject_text(input: &str, subject: BlockSubject) -> Text {
    let id = identifier_string(input, subject.span);
    if subject.tag { format!("#{id}") } else { id }.into_text()
}

impl ErrorText for BlockError {
    fn text(&self, input: &str) -> Text {
        use BlockError as E;
        match *self {
            E::BlockIdInvalid(id) => Text::translate(
                "argument.block.id.invalid",
                vec![identifier_text(input, id)],
            ),
            E::BlockTagUnknown(id) => Text::translate(
                "arguments.block.tag.unknown",
                vec![identifier_text(input, id)],
            ),
            E::PropertyUnknown { block, name } => Text::translate(
                "argument.block.property.unknown",
                vec![identifier_text(input, block), string_text(input, name)],
            ),
            E::PropertyDuplicate { name, subject } => Text::translate(
                "argument.block.property.duplicate",
                vec![string_text(input, name), subject_text(input, subject)],
            ),
            E::PropertyNoValue { name, subject } => Text::translate(
                "argument.block.property.novalue",
                vec![string_text(input, name), subject_text(input, subject)],
            ),
            E::PropertyInvalid { block, value, name } => Text::translate(
                "argument.block.property.invalid",
                vec![
                    identifier_text(input, block),
                    string_text(input, value),
                    string_text(input, name),
                ],
            ),
            E::PropertyUnclosed => Text::translate("argument.block.property.unclosed", vec![]),
            E::Nbt(error) => snbt_text(input, error),
            E::Resource(error) => error.text(input),
        }
    }
}
