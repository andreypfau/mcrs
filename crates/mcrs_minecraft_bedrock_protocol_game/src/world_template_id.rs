use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

/// The `uuid` of the template of a world: the zero UUID when the world is not from a template.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct WorldTemplateId {
    /// The 64 most significant bits.
    pub most_significant: u64,
    /// The 64 least significant bits.
    pub least_significant: u64,
}
