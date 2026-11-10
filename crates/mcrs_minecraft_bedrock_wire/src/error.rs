//! Bedrock wire error definitions.

use core::fmt;

/// Errors that can occur during Bedrock wire encoding and decoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Unexpected end of input while decoding.
    UnexpectedEof,
    /// Malformed varint byte stream.
    InvalidVarInt,
    /// Out of range bits present in final varint byte.
    VarIntOverflow,
    /// String bytes are not valid UTF-8.
    InvalidUtf8,
    /// Boolean value is not 0 or 1.
    InvalidBool,
    /// Trailing unread bytes remain in the slice.
    TrailingBytes,
    /// A length does not fit its length prefix or the address space.
    LengthOverflow,
    /// A packet header carries another packet id than the one decoded.
    UnexpectedPacketId {
        /// The id of the packet the caller decodes.
        expected: u32,
        /// The id in the header.
        actual: u32,
    },
    /// An enum field holds a value outside its known set.
    UnknownEnumValue(i64),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::InvalidVarInt => f.write_str("invalid variable-length integer encoding"),
            Self::VarIntOverflow => f.write_str("variable-length integer overflow"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8 byte sequence in string"),
            Self::InvalidBool => f.write_str("invalid boolean byte: expected 0 or 1"),
            Self::TrailingBytes => f.write_str("trailing unconsumed bytes after decoding"),
            Self::LengthOverflow => f.write_str("length exceeds its length prefix"),
            Self::UnexpectedPacketId { expected, actual } => {
                write!(f, "unexpected packet id {actual}, expected {expected}")
            }
            Self::UnknownEnumValue(value) => write!(f, "unknown enum value {value}"),
        }
    }
}

impl core::error::Error for Error {}
