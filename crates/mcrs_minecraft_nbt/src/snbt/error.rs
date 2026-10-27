use core::fmt;

/// A range of byte offsets in the text that was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn text(self, input: &str) -> &str {
        input.get(self.start..self.end).unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerType {
    Byte,
    Short,
    Int,
    Long,
}

impl IntegerType {
    pub const fn bits(self) -> u32 {
        match self {
            IntegerType::Byte => 8,
            IntegerType::Short => 16,
            IntegerType::Int => 32,
            IntegerType::Long => 64,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            IntegerType::Byte => "byte",
            IntegerType::Short => "short",
            IntegerType::Int => "int",
            IntegerType::Long => "long",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnbtErrorKind {
    /// The code units the token accepts, joined with `|`.
    LiteralIncorrect(&'static str),
    ExpectedUnquotedString,
    ExpectedDecimalNumeral,
    ExpectedHexNumeral,
    ExpectedBinaryNumeral,
    UnderscoreNotAllowed,
    LeadingZeroNotAllowed,
    InvalidStringContents,
    ExpectedHexEscape(u8),
    InvalidCharacterName,
    InvalidCodepoint(u32),
    NumberParseFailure {
        number: Span,
        ty: IntegerType,
    },
    ExpectedNonNegativeNumber,
    InvalidArrayElementType,
    InfinityNotAllowed,
    EmptyKey,
    InvalidUnquotedStart,
    NoSuchOperation {
        name: Span,
        arguments: usize,
    },
    ExpectedNumberOrBoolean,
    ExpectedStringUuid,
    ExpectedCompound,
    Trailing,
    TooDeep,
}

impl SnbtErrorKind {
    pub const fn key(self) -> &'static str {
        use SnbtErrorKind as K;
        match self {
            K::LiteralIncorrect(_) => "argument.literal.incorrect",
            K::ExpectedUnquotedString => "snbt.parser.expected_unquoted_string",
            K::ExpectedDecimalNumeral => "snbt.parser.expected_decimal_numeral",
            K::ExpectedHexNumeral => "snbt.parser.expected_hex_numeral",
            K::ExpectedBinaryNumeral => "snbt.parser.expected_binary_numeral",
            K::UnderscoreNotAllowed => "snbt.parser.underscore_not_allowed",
            K::LeadingZeroNotAllowed => "snbt.parser.leading_zero_not_allowed",
            K::InvalidStringContents => "snbt.parser.invalid_string_contents",
            K::ExpectedHexEscape(_) => "snbt.parser.expected_hex_escape",
            K::InvalidCharacterName => "snbt.parser.invalid_character_name",
            K::InvalidCodepoint(_) => "snbt.parser.invalid_codepoint",
            K::NumberParseFailure { .. } => "snbt.parser.number_parse_failure",
            K::ExpectedNonNegativeNumber => "snbt.parser.expected_non_negative_number",
            K::InvalidArrayElementType => "snbt.parser.invalid_array_element_type",
            K::InfinityNotAllowed => "snbt.parser.infinity_not_allowed",
            K::EmptyKey => "snbt.parser.empty_key",
            K::InvalidUnquotedStart => "snbt.parser.invalid_unquoted_start",
            K::NoSuchOperation { .. } => "snbt.parser.no_such_operation",
            K::ExpectedNumberOrBoolean => "snbt.parser.expected_number_or_boolean",
            K::ExpectedStringUuid => "snbt.parser.expected_string_uuid",
            K::ExpectedCompound => "argument.nbt.expected.compound",
            K::Trailing => "argument.nbt.trailing",
            K::TooDeep => "arguments.nbtpath.too_deep",
        }
    }

    /// Writes the key argument, if the key has one; the text of a span comes from `input`.
    pub fn write_argument(self, input: &str, out: &mut impl fmt::Write) -> fmt::Result {
        use SnbtErrorKind as K;
        match self {
            K::LiteralIncorrect(set) => out.write_str(set),
            K::ExpectedHexEscape(length) => write!(out, "{length}"),
            K::InvalidCodepoint(code) => write!(out, "U+{code:08X}"),
            K::NumberParseFailure { number, ty } => write!(
                out,
                "{} does not fit in a {}",
                number.text(input),
                ty.name()
            ),
            K::NoSuchOperation { name, arguments } => {
                write!(out, "{}/{arguments}", name.text(input))
            }
            _ => Ok(()),
        }
    }
}

/// An error of the read: a kind and the byte offset in the text where it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnbtError {
    pub kind: SnbtErrorKind,
    pub position: usize,
}

impl fmt::Display for SnbtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}", self.kind.key(), self.position)
    }
}

impl core::error::Error for SnbtError {}
