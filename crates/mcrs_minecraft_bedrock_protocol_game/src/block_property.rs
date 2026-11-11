use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

use crate::network_nbt::NetworkNbt;

/// A custom block of [`StartGame`](crate::StartGame).
#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
pub struct BlockProperty<'a> {
    /// The namespaced name of the block.
    pub name: &'a str,
    /// The properties of the block.
    pub properties: NetworkNbt,
}
