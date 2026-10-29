use crate::identifier::Identifier;
use core::fmt;
use mcrs_command::{ErrorKind, LibraryError, StringSpan, StringValue, SyntaxError};
use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::text::{Color, IntoText, Text, TranslateArg};
use mcrs_minecraft_snbt::SnbtError;
use mcrs_text_span::Span;

/// The errors of an identifier and of the arguments that read a registry entry or a tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceError {
    /// The text is no identifier.
    IdInvalid,
    /// The registry holds no entry with the identifier at `span`.
    ResourceNotFound {
        span: Span,
        registry: StaticResourceLocation,
    },
    /// The registry holds no tag with the identifier at `span`.
    ResourceTagNotFound {
        span: Span,
        registry: StaticResourceLocation,
    },
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ResourceError::IdInvalid => "invalid identifier",
            ResourceError::ResourceNotFound { .. } => "unknown registry entry",
            ResourceError::ResourceTagNotFound { .. } => "unknown registry tag",
        })
    }
}

impl core::error::Error for ResourceError {}

/// The errors of the arguments that read one word of a fixed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeywordError {
    /// The word at the span names no game mode.
    GameModeInvalid(Span),
    /// The word at the span names no entity anchor.
    AnchorInvalid(Span),
    /// The word at the span names no value of the set.
    EnumInvalid(Span),
}

impl fmt::Display for KeywordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            KeywordError::GameModeInvalid(_) => "unknown game mode",
            KeywordError::AnchorInvalid(_) => "unknown entity anchor",
            KeywordError::EnumInvalid(_) => "unknown value",
        })
    }
}

impl core::error::Error for KeywordError {}

/// The translated text of an argument error. Text from the command line is a span of `input`.
pub trait ErrorText {
    fn text(&self, input: &str) -> Text;
}

pub fn string_text(input: &str, string: StringSpan) -> Text {
    StringValue::new(input, string)
        .chars()
        .collect::<String>()
        .into_text()
}

pub fn span_text(input: &str, span: Span) -> Text {
    span.text(input).to_owned().into_text()
}

pub fn identifier_text(input: &str, span: Span) -> Text {
    identifier_string(input, span).into_text()
}

pub fn identifier_string(input: &str, span: Span) -> String {
    let text = span.text(input);
    Identifier::parse(text).map_or_else(|| text.to_owned(), |id| id.to_string())
}

fn bound(key: &'static str, limit: NbtTag, value: NbtTag) -> Text {
    Text::translate_args(
        key,
        vec![TranslateArg::Number(limit), TranslateArg::Number(value)],
    )
}

fn library_text(input: &str, error: LibraryError) -> Text {
    use LibraryError::*;
    match error {
        DoubleTooLow { value, min } => bound(
            "argument.double.low",
            NbtTag::Double(min),
            NbtTag::Double(value),
        ),
        DoubleTooHigh { value, max } => bound(
            "argument.double.big",
            NbtTag::Double(max),
            NbtTag::Double(value),
        ),
        FloatTooLow { value, min } => bound(
            "argument.float.low",
            NbtTag::Float(min),
            NbtTag::Float(value),
        ),
        FloatTooHigh { value, max } => bound(
            "argument.float.big",
            NbtTag::Float(max),
            NbtTag::Float(value),
        ),
        IntegerTooLow { value, min } => {
            bound("argument.integer.low", NbtTag::Int(min), NbtTag::Int(value))
        }
        IntegerTooHigh { value, max } => {
            bound("argument.integer.big", NbtTag::Int(max), NbtTag::Int(value))
        }
        LongTooLow { value, min } => {
            bound("argument.long.low", NbtTag::Long(min), NbtTag::Long(value))
        }
        LongTooHigh { value, max } => {
            bound("argument.long.big", NbtTag::Long(max), NbtTag::Long(value))
        }
        ExpectedEndOfQuote => Text::translate("parsing.quote.expected.end", vec![]),
        InvalidEscape(c) => {
            Text::translate("parsing.quote.escape", vec![c.to_string().into_text()])
        }
        InvalidBool(text) => {
            Text::translate("parsing.bool.invalid", vec![string_text(input, text)])
        }
        ExpectedBool => Text::translate("parsing.bool.expected", vec![]),
        InvalidInt(token) => Text::translate("parsing.int.invalid", vec![span_text(input, token)]),
        ExpectedInt => Text::translate("parsing.int.expected", vec![]),
        InvalidLong(token) => {
            Text::translate("parsing.long.invalid", vec![span_text(input, token)])
        }
        ExpectedLong => Text::translate("parsing.long.expected", vec![]),
        InvalidDouble(token) => {
            Text::translate("parsing.double.invalid", vec![span_text(input, token)])
        }
        ExpectedDouble => Text::translate("parsing.double.expected", vec![]),
        InvalidFloat(token) => {
            Text::translate("parsing.float.invalid", vec![span_text(input, token)])
        }
        ExpectedFloat => Text::translate("parsing.float.expected", vec![]),
        ExpectedSymbol(c) => Text::translate("parsing.expected", vec![c.to_string().into_text()]),
        UnknownCommand => Text::translate("command.unknown.command", vec![]),
        UnknownArgument => Text::translate("command.unknown.argument", vec![]),
        ExpectedSeparator => Text::translate("command.expected.separator", vec![]),
        _ => error.to_string().into_text(),
    }
}

impl ErrorText for ResourceError {
    fn text(&self, input: &str) -> Text {
        let registry = |registry: StaticResourceLocation| registry.as_str().to_owned().into_text();
        match *self {
            ResourceError::IdInvalid => Text::translate("argument.id.invalid", vec![]),
            ResourceError::ResourceNotFound { span, registry: r } => Text::translate(
                "argument.resource.not_found",
                vec![identifier_text(input, span), registry(r)],
            ),
            ResourceError::ResourceTagNotFound { span, registry: r } => Text::translate(
                "argument.resource_tag.not_found",
                vec![identifier_text(input, span), registry(r)],
            ),
        }
    }
}

impl ErrorText for KeywordError {
    fn text(&self, input: &str) -> Text {
        match *self {
            KeywordError::GameModeInvalid(word) => {
                Text::translate("argument.gamemode.invalid", vec![span_text(input, word)])
            }
            KeywordError::AnchorInvalid(word) => {
                Text::translate("argument.anchor.invalid", vec![span_text(input, word)])
            }
            KeywordError::EnumInvalid(word) => {
                Text::translate("argument.enum.invalid", vec![span_text(input, word)])
            }
        }
    }
}

pub fn snbt_text(input: &str, error: SnbtError) -> Text {
    let mut with = Vec::new();
    let mut argument = String::new();
    if error.kind.write_argument(input, &mut argument).is_ok() && !argument.is_empty() {
        with.push(argument.into_text());
    }
    Text::translate(error.kind.key(), with)
}

/// The message of a syntax error, without its context text.
pub fn error_message<E: ErrorText>(input: &str, error: &SyntaxError<E>) -> Text {
    match &error.kind {
        ErrorKind::Library(kind) => library_text(input, *kind),
        ErrorKind::Argument(kind) => kind.text(input),
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
pub fn parse_error_line<E: ErrorText>(input: &str, error: &SyntaxError<E>) -> Text {
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
fn error_context(input: &str, position: usize) -> Text {
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
pub fn error_feedback<E: ErrorText>(input: &str, error: &SyntaxError<E>) -> [Text; 2] {
    [
        error_message(input, error).color(Color::RED),
        error_context(input, error.position),
    ]
}
