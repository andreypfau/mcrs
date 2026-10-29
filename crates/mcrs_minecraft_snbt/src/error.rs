use core::fmt;

use mcrs_text_span::Span;

/// The integer types of SNBT, by their suffix letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerType {
    /// `b`: 8 bits.
    Byte,
    /// `s`: 16 bits.
    Short,
    /// `i` or no letter: 32 bits.
    Int,
    /// `l`: 64 bits.
    Long,
}

impl IntegerType {
    /// The width of the type in bits.
    pub const fn bits(self) -> u32 {
        match self {
            IntegerType::Byte => 8,
            IntegerType::Short => 16,
            IntegerType::Int => 32,
            IntegerType::Long => 64,
        }
    }

    /// The name of the type in an error message.
    pub const fn name(self) -> &'static str {
        match self {
            IntegerType::Byte => "byte",
            IntegerType::Short => "short",
            IntegerType::Int => "int",
            IntegerType::Long => "long",
        }
    }
}

/// What an SNBT read rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnbtErrorKind {
    /// The code units the token accepts, joined with `|`.
    LiteralIncorrect(&'static str),
    /// An unquoted string is expected.
    ExpectedUnquotedString,
    /// A decimal digit is expected.
    ExpectedDecimalNumeral,
    /// A hexadecimal digit is expected.
    ExpectedHexNumeral,
    /// A binary digit is expected.
    ExpectedBinaryNumeral,
    /// A run of digits starts or ends with `_`.
    UnderscoreNotAllowed,
    /// A decimal integer has a leading zero.
    LeadingZeroNotAllowed,
    /// A quoted string holds a character that it may not hold.
    InvalidStringContents,
    /// An escape takes this number of hexadecimal digits.
    ExpectedHexEscape(u8),
    /// A `\N{...}` escape names no character.
    InvalidCharacterName,
    /// An escape names no Unicode scalar value.
    InvalidCodepoint(u32),
    /// The integer at `number` does not fit in `ty`.
    NumberParseFailure { number: Span, ty: IntegerType },
    /// An unsigned integer has a `-`.
    ExpectedNonNegativeNumber,
    /// The type of an element does not fit the array.
    InvalidArrayElementType,
    /// A floating-point value is infinite.
    InfinityNotAllowed,
    /// A key of a compound is empty.
    EmptyKey,
    /// An unquoted string starts with a character that it may not start with.
    InvalidUnquotedStart,
    /// No operation has the name at `name` and this number of arguments.
    NoSuchOperation { name: Span, arguments: usize },
    /// An operation takes a number or a boolean.
    ExpectedNumberOrBoolean,
    /// An operation takes a UUID string.
    ExpectedStringUuid,
    /// A compound is expected.
    ExpectedCompound,
    /// Text follows the value.
    Trailing,
    /// The value nests too deep.
    TooDeep,
}

impl SnbtErrorKind {
    /// The translation key of the error.
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

    /// Writes the key argument, if the key has one; the text of a span comes from `input`.
    pub fn write_argument(self, input: &str, out: &mut impl fmt::Write) -> fmt::Result {
        use SnbtErrorKind::*;
        match self {
            LiteralIncorrect(set) => out.write_str(set),
            ExpectedHexEscape(length) => write!(out, "{length}"),
            InvalidCodepoint(code) => write!(out, "U+{code:08X}"),
            NumberParseFailure { number, ty } => write!(
                out,
                "{} does not fit in a {}",
                number.text(input),
                ty.name()
            ),
            NoSuchOperation { name, arguments } => {
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
