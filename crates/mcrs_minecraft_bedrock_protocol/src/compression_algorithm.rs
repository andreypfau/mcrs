use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

/// The compression algorithm that `NetworkSettings` selects for both directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = u16)]
pub enum CompressionAlgorithm {
    /// Raw deflate (RFC 1951), with no zlib header and no checksum.
    Deflate = 0x0000,
    /// A Snappy block, with no frame format.
    Snappy = 0x0001,
    /// No compression: each batch has the marker `0xff`.
    Disabled = 0xffff,
}
