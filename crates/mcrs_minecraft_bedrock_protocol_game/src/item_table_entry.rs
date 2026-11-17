use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct ItemTableEntry<'a> {
    pub name: &'a str,
    pub network_id: i16,
    pub component_based: bool,
}
