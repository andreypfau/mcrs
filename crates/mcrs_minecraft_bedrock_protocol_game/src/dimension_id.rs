use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum DimensionId {
    Overworld = 0,
    Nether = 1,
    End = 2,
}
