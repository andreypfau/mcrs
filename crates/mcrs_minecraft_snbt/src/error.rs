use core::fmt;

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
        start: usize,
        end: usize,
        ty: IntegerType,
    },
    ExpectedNonNegativeNumber,
    InvalidArrayElementType,
    InfinityNotAllowed,
    EmptyKey,
    InvalidUnquotedStart,
    NoSuchOperation {
        start: usize,
        end: usize,
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
        use SnbtErrorKind::*;
        match self {
            LiteralIncorrect(_) => "argument.literal.incorrect",
            ExpectedUnquotedString => "snbt.parser.expected_unquoted_string",
            ExpectedDecimalNumeral => "snbt.parser.expected_decimal_numeral",
            ExpectedHexNumeral => "snbt.parser.expected_hex_numeral",
            ExpectedBinaryNumeral => "snbt.parser.expected_binary_numeral",
            UnderscoreNotAllowed => "snbt.parser.underscore_not_allowed",
            LeadingZeroNotAllowed => "snbt.parser.leading_zero_not_allowed",
            InvalidStringContents => "snbt.parser.invalid_string_contents",
            ExpectedHexEscape(_) => "snbt.parser.expected_hex_escape",
            InvalidCharacterName => "snbt.parser.invalid_character_name",
            InvalidCodepoint(_) => "snbt.parser.invalid_codepoint",
            NumberParseFailure { .. } => "snbt.parser.number_parse_failure",
            ExpectedNonNegativeNumber => "snbt.parser.expected_non_negative_number",
            InvalidArrayElementType => "snbt.parser.invalid_array_element_type",
            InfinityNotAllowed => "snbt.parser.infinity_not_allowed",
            EmptyKey => "snbt.parser.empty_key",
            InvalidUnquotedStart => "snbt.parser.invalid_unquoted_start",
            NoSuchOperation { .. } => "snbt.parser.no_such_operation",
            ExpectedNumberOrBoolean => "snbt.parser.expected_number_or_boolean",
            ExpectedStringUuid => "snbt.parser.expected_string_uuid",
            ExpectedCompound => "argument.nbt.expected.compound",
            Trailing => "argument.nbt.trailing",
            TooDeep => "arguments.nbtpath.too_deep",
        }
    }

    pub fn write_argument(self, input: &str, out: &mut impl fmt::Write) -> fmt::Result {
        use SnbtErrorKind::*;
        match self {
            LiteralIncorrect(set) => out.write_str(set),
            ExpectedHexEscape(length) => write!(out, "{length}"),
            InvalidCodepoint(code) => write!(out, "U+{code:08X}"),
            NumberParseFailure { start, end, ty } => write!(
                out,
                "{} does not fit in a {}",
                input.get(start..end).unwrap_or_default(),
                ty.name()
            ),
            NoSuchOperation {
                start,
                end,
                arguments,
            } => {
                let name = input.get(start..end).unwrap_or_default();
                write!(out, "{name}/{arguments}")
            }
            _ => Ok(()),
        }
    }
}

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
