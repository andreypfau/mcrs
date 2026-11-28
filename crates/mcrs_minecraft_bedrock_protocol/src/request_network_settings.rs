use mcrs_minecraft_bedrock_wire::{BeI32, BedrockDecode, BedrockEncode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0xc1)]
pub struct RequestNetworkSettings {
    pub protocol_version: BeI32,
}
