use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

/// The compression algorithm that `NetworkSettings` selects for a connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompressionAlgorithm {
    /// Raw deflate.
    #[default]
    Deflate,
    /// Snappy.
    Snappy,
    /// No compression.
    None,
}

impl CompressionAlgorithm {
    const fn id(self) -> u16 {
        match self {
            Self::Deflate => 0,
            Self::Snappy => 1,
            Self::None => 0xFFFF,
        }
    }

    pub(crate) const fn batch_marker(self) -> u8 {
        match self {
            Self::Deflate => 0x00,
            Self::Snappy => 0x01,
            Self::None => 0xFF,
        }
    }
}

impl BedrockEncode for CompressionAlgorithm {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.id().encode(w)
    }
}

impl<'a> BedrockDecode<'a> for CompressionAlgorithm {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        match u16::decode(r)? {
            0 => Ok(Self::Deflate),
            1 => Ok(Self::Snappy),
            0xFFFF => Ok(Self::None),
            other => Err(Error::UnknownEnumValue(i64::from(other))),
        }
    }
}
