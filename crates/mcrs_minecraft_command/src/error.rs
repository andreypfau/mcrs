use crate::identifier::Identifier;
use mcrs_command::{ErrorKind, LibraryError, StringSpan, StringValue, SyntaxError};
use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text, TranslateArg};
use mcrs_minecraft_snbt::SnbtError;
use mcrs_text_span::Span;

/// The syntax errors of the vanilla argument types. Text from the command line is a span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgumentError {
    IdInvalid,
    ResourceNotFound {
        id: Span,
        registry: StaticResourceLocation,
    },
    ResourceTagNotFound {
        id: Span,
        registry: StaticResourceLocation,
    },
    TimeInvalidUnit,
    TickCountTooLow {
        min: i32,
        ticks: i32,
    },
    GameModeInvalid(Span),
    AnchorInvalid(Span),
    PosMixed,
    PosMissingDouble,
    Pos3dIncomplete,
    RotationIncomplete,
    SelectorNotAllowed,
    SelectorMissing,
    SelectorUnknown(char),
    EntityInvalid,
    OptionsUnknown(StringSpan),
    OptionsInapplicable(StringSpan),
    OptionsValueless(StringSpan),
    OptionsUnterminated,
    DistanceNegative,
    LevelNegative,
    LimitTooSmall,
    SortIrreversible(Span),
    ModeInvalid(Span),
    TypeInvalid(Span),
    RangeEmpty,
    RangeSwapped,
    PlayerTooMany,
    EntityTooMany,
    PlayerEntities,
    PosMissingInt,
    SwizzleInvalid,
    EnumInvalid(Span),
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

pub type CommandSyntaxError = SyntaxError<ArgumentError>;

/// A library argument type has no errors of its own.
impl From<core::convert::Infallible> for ArgumentError {
    fn from(never: core::convert::Infallible) -> Self {
        match never {}
    }
}

fn string(input: &str, string: StringSpan) -> Text {
    StringValue::new(input, string)
        .chars()
        .collect::<String>()
        .into_text()
}

fn span(input: &str, span: Span) -> Text {
    span.text(input).to_owned().into_text()
}

fn identifier(input: &str, span: Span) -> Text {
    identifier_string(input, span).into_text()
}

fn identifier_string(input: &str, span: Span) -> String {
    let text = span.text(input);
    Identifier::parse(text).map_or_else(|| text.to_owned(), |id| id.to_string())
}

fn subject_text(input: &str, subject: BlockSubject) -> Text {
    let id = identifier_string(input, subject.span);
    if subject.tag { format!("#{id}") } else { id }.into_text()
}

fn bound(key: &'static str, limit: NbtTag, value: NbtTag) -> Text {
    Text::translate_args(
        key,
        vec![TranslateArg::Number(limit), TranslateArg::Number(value)],
    )
}

fn library_text(input: &str, error: LibraryError) -> Text {
    use LibraryError as E;
    match error {
        E::DoubleTooLow { value, min } => bound(
            "argument.double.low",
            NbtTag::Double(min),
            NbtTag::Double(value),
        ),
        E::DoubleTooHigh { value, max } => bound(
            "argument.double.big",
            NbtTag::Double(max),
            NbtTag::Double(value),
        ),
        E::FloatTooLow { value, min } => bound(
            "argument.float.low",
            NbtTag::Float(min),
            NbtTag::Float(value),
        ),
        E::FloatTooHigh { value, max } => bound(
            "argument.float.big",
            NbtTag::Float(max),
            NbtTag::Float(value),
        ),
        E::IntegerTooLow { value, min } => {
            bound("argument.integer.low", NbtTag::Int(min), NbtTag::Int(value))
        }
        E::IntegerTooHigh { value, max } => {
            bound("argument.integer.big", NbtTag::Int(max), NbtTag::Int(value))
        }
        E::LongTooLow { value, min } => {
            bound("argument.long.low", NbtTag::Long(min), NbtTag::Long(value))
        }
        E::LongTooHigh { value, max } => {
            bound("argument.long.big", NbtTag::Long(max), NbtTag::Long(value))
        }
        E::ExpectedEndOfQuote => Text::translate("parsing.quote.expected.end", vec![]),
        E::InvalidEscape(c) => {
            Text::translate("parsing.quote.escape", vec![c.to_string().into_text()])
        }
        E::InvalidBool(text) => Text::translate("parsing.bool.invalid", vec![string(input, text)]),
        E::ExpectedBool => Text::translate("parsing.bool.expected", vec![]),
        E::InvalidInt(token) => Text::translate("parsing.int.invalid", vec![span(input, token)]),
        E::ExpectedInt => Text::translate("parsing.int.expected", vec![]),
        E::InvalidLong(token) => Text::translate("parsing.long.invalid", vec![span(input, token)]),
        E::ExpectedLong => Text::translate("parsing.long.expected", vec![]),
        E::InvalidDouble(token) => {
            Text::translate("parsing.double.invalid", vec![span(input, token)])
        }
        E::ExpectedDouble => Text::translate("parsing.double.expected", vec![]),
        E::InvalidFloat(token) => {
            Text::translate("parsing.float.invalid", vec![span(input, token)])
        }
        E::ExpectedFloat => Text::translate("parsing.float.expected", vec![]),
        E::ExpectedSymbol(c) => {
            Text::translate("parsing.expected", vec![c.to_string().into_text()])
        }
        E::UnknownCommand => Text::translate("command.unknown.command", vec![]),
        E::UnknownArgument => Text::translate("command.unknown.argument", vec![]),
        E::ExpectedSeparator => Text::translate("command.expected.separator", vec![]),
    }
}

pub fn argument_text(input: &str, error: ArgumentError) -> Text {
    use ArgumentError as E;
    let registry = |registry: StaticResourceLocation| registry.as_str().to_owned().into_text();
    match error {
        E::IdInvalid => Text::translate("argument.id.invalid", vec![]),
        E::ResourceNotFound { id, registry: r } => Text::translate(
            "argument.resource.not_found",
            vec![identifier(input, id), registry(r)],
        ),
        E::ResourceTagNotFound { id, registry: r } => Text::translate(
            "argument.resource_tag.not_found",
            vec![identifier(input, id), registry(r)],
        ),
        E::TimeInvalidUnit => Text::translate("argument.time.invalid_unit", vec![]),
        E::TickCountTooLow { min, ticks } => Text::translate_args(
            "argument.time.tick_count_too_low",
            vec![min.into(), ticks.into()],
        ),
        E::GameModeInvalid(word) => {
            Text::translate("argument.gamemode.invalid", vec![span(input, word)])
        }
        E::AnchorInvalid(word) => {
            Text::translate("argument.anchor.invalid", vec![span(input, word)])
        }
        E::PosMixed => Text::translate("argument.pos.mixed", vec![]),
        E::PosMissingDouble => Text::translate("argument.pos.missing.double", vec![]),
        E::Pos3dIncomplete => Text::translate("argument.pos3d.incomplete", vec![]),
        E::RotationIncomplete => Text::translate("argument.rotation.incomplete", vec![]),
        E::SelectorNotAllowed => Text::translate("argument.entity.selector.not_allowed", vec![]),
        E::SelectorMissing => Text::translate("argument.entity.selector.missing", vec![]),
        E::SelectorUnknown(kind) => Text::translate(
            "argument.entity.selector.unknown",
            vec![format!("@{kind}").into_text()],
        ),
        E::EntityInvalid => Text::translate("argument.entity.invalid", vec![]),
        E::OptionsUnknown(name) => {
            Text::translate("argument.entity.options.unknown", vec![string(input, name)])
        }
        E::OptionsInapplicable(name) => Text::translate(
            "argument.entity.options.inapplicable",
            vec![string(input, name)],
        ),
        E::OptionsValueless(name) => Text::translate(
            "argument.entity.options.valueless",
            vec![string(input, name)],
        ),
        E::OptionsUnterminated => Text::translate("argument.entity.options.unterminated", vec![]),
        E::DistanceNegative => Text::translate("argument.entity.options.distance.negative", vec![]),
        E::LevelNegative => Text::translate("argument.entity.options.level.negative", vec![]),
        E::LimitTooSmall => Text::translate("argument.entity.options.limit.toosmall", vec![]),
        E::SortIrreversible(word) => Text::translate(
            "argument.entity.options.sort.irreversible",
            vec![span(input, word)],
        ),
        E::ModeInvalid(word) => Text::translate(
            "argument.entity.options.mode.invalid",
            vec![span(input, word)],
        ),
        E::TypeInvalid(id) => Text::translate(
            "argument.entity.options.type.invalid",
            vec![identifier(input, id)],
        ),
        E::RangeEmpty => Text::translate("argument.range.empty", vec![]),
        E::RangeSwapped => Text::translate("argument.range.swapped", vec![]),
        E::PlayerTooMany => Text::translate("argument.player.toomany", vec![]),
        E::EntityTooMany => Text::translate("argument.entity.toomany", vec![]),
        E::PlayerEntities => Text::translate("argument.player.entities", vec![]),
        E::PosMissingInt => Text::translate("argument.pos.missing.int", vec![]),
        E::SwizzleInvalid => Text::translate("arguments.swizzle.invalid", vec![]),
        E::EnumInvalid(word) => Text::translate("argument.enum.invalid", vec![span(input, word)]),
        E::BlockIdInvalid(id) => {
            Text::translate("argument.block.id.invalid", vec![identifier(input, id)])
        }
        E::BlockTagUnknown(id) => {
            Text::translate("arguments.block.tag.unknown", vec![identifier(input, id)])
        }
        E::PropertyUnknown { block, name } => Text::translate(
            "argument.block.property.unknown",
            vec![identifier(input, block), string(input, name)],
        ),
        E::PropertyDuplicate { name, subject } => Text::translate(
            "argument.block.property.duplicate",
            vec![string(input, name), subject_text(input, subject)],
        ),
        E::PropertyNoValue { name, subject } => Text::translate(
            "argument.block.property.novalue",
            vec![string(input, name), subject_text(input, subject)],
        ),
        E::PropertyInvalid { block, value, name } => Text::translate(
            "argument.block.property.invalid",
            vec![
                identifier(input, block),
                string(input, value),
                string(input, name),
            ],
        ),
        E::PropertyUnclosed => Text::translate("argument.block.property.unclosed", vec![]),
        E::Nbt(error) => snbt_text(input, error),
    }
}

fn snbt_text(input: &str, error: SnbtError) -> Text {
    let mut with = Vec::new();
    let mut argument = String::new();
    if error.kind.write_argument(input, &mut argument).is_ok() && !argument.is_empty() {
        with.push(argument.into_text());
    }
    Text::translate(error.kind.key(), with)
}

/// The message of a syntax error, without its context text.
pub fn error_message(input: &str, error: &CommandSyntaxError) -> Text {
    match error.kind {
        ErrorKind::Library(kind) => library_text(input, kind),
        ErrorKind::Argument(kind) => argument_text(input, kind),
    }
}

const CONTEXT_AMOUNT: usize = 10;

/// The context text of an error (CMD-ERR-003): up to ten code units before the position, after
/// an ellipsis when more come before them.
fn context_text(input: &str, position: usize) -> String {
    let position = position.min(input.len());
    let units: Vec<u16> = input
        .get(..position)
        .unwrap_or_default()
        .encode_utf16()
        .collect();
    let mut from = units.len().saturating_sub(CONTEXT_AMOUNT);
    if units
        .get(from)
        .is_some_and(|&unit| (0xdc00..0xe000).contains(&unit))
    {
        from += 1;
    }
    let before = String::from_utf16_lossy(units.get(from..).unwrap_or_default());
    if units.len() > CONTEXT_AMOUNT {
        format!("...{before}")
    } else {
        before
    }
}

/// The error line the chat screen shows over a command input (CCMD-HINT-002). `input` holds the
/// `/`, and so does the position.
pub fn parse_error_line(input: &str, error: &CommandSyntaxError) -> Text {
    let message = error_message(input, error);
    let position = mcrs_text_span::utf16_position(input, error.position);
    let mut context = context_text(input, error.position);
    context.push_str("<--[HERE]");
    Text::translate(
        "command.context.parse_error",
        vec![
            message,
            position.to_string().into_text(),
            context.into_text(),
        ],
    )
}

/// The context line of an error (CMD-ERR-003, CMDP-EXEC-003): up to ten code units before the
/// position, then the rest of the line in red and underlined, then `command.context.here`.
pub(crate) fn error_context(input: &str, position: usize) -> Text {
    let position = position.min(input.len());
    let mut context = context_text(input, position).into_text();
    if let Some(rest) = input.get(position..).filter(|rest| !rest.is_empty()) {
        context = context.add_child(rest.to_owned().color(Color::RED).underlined());
    }
    context
        .add_child(
            Text::translate("command.context.here", vec![])
                .color(Color::RED)
                .italic(),
        )
        .color(Color::GRAY)
        .on_click_suggest_command(format!("/{input}"))
}

/// The two system messages a player receives for a command line that does not parse.
pub fn error_feedback(input: &str, error: &CommandSyntaxError) -> [Text; 2] {
    [
        error_message(input, error).color(Color::RED),
        error_context(input, error.position),
    ]
}
