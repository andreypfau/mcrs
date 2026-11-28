use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct WorldTemplateId {
    pub most_significant: u64,
    pub least_significant: u64,
}
