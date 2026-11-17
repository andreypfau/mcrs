use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

use crate::network_nbt::NetworkNbt;

#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct BlockProperty<'a> {
    pub name: &'a str,
    pub properties: NetworkNbt,
}
