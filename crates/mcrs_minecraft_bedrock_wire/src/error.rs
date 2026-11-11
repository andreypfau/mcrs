use core::fmt;

/// A failure to encode or decode a Bedrock wire value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Unexpected end of input while decoding.
    UnexpectedEof,
    /// A varint has more bytes or bits than its integer holds.
    VarIntOverflow,
    /// String bytes are not valid UTF-8.
    InvalidUtf8,
    /// Trailing unread bytes remain in the slice.
    TrailingBytes,
    /// A length does not fit its length prefix or the address space.
    LengthOverflow,
    /// A packet header carries another packet id than the one decoded.
    UnexpectedPacketId {
        /// The id of the packet the caller decodes.
        expected: u16,
        /// The id in the header.
        actual: u16,
    },
    /// A packet header field does not fit its bits.
    PacketHeaderOverflow,
    /// An enum field holds a value outside its known set.
    UnknownEnumValue(i64),
    /// A network NBT value is not valid.
    Nbt,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::VarIntOverflow => f.write_str("variable-length integer overflow"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8 byte sequence in string"),
            Self::TrailingBytes => f.write_str("trailing unconsumed bytes after decoding"),
            Self::LengthOverflow => f.write_str("length exceeds its length prefix"),
            Self::UnexpectedPacketId { expected, actual } => {
                write!(f, "unexpected packet id {actual}, expected {expected}")
            }
            Self::PacketHeaderOverflow => f.write_str("packet header field exceeds its bits"),
            Self::UnknownEnumValue(value) => write!(f, "unknown enum value {value}"),
            Self::Nbt => f.write_str("invalid network NBT value"),
        }
    }
}

impl core::error::Error for Error {}
