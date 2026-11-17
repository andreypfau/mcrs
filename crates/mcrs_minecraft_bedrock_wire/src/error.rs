use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    UnexpectedEof,
    VarIntOverflow,
    InvalidUtf8,
    TrailingBytes,
    LengthOverflow,
    UnexpectedPacketId { expected: u16, actual: u16 },
    PacketHeaderOverflow,
    UnknownEnumValue(i64),
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
