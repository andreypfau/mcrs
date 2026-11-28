use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = u16)]
pub enum CompressionAlgorithm {
    Deflate = 0x0000,
    Snappy = 0x0001,
    Disabled = 0xffff,
}
