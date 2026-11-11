use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32};

/// The dimension of a packet, as a varint32.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum DimensionId {
    /// The overworld.
    Overworld = 0,
    /// The nether.
    Nether = 1,
    /// The end.
    End = 2,
}
