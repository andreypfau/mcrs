use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

/// An item of the item table of [`StartGame`](crate::StartGame).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct ItemTableEntry<'a> {
    /// The namespaced name, for example `minecraft:stick`.
    pub name: &'a str,
    /// The id that the item fields of other packets carry for this connection.
    pub network_id: i16,
    /// Whether the item is a custom item with components.
    pub component_based: bool,
}
