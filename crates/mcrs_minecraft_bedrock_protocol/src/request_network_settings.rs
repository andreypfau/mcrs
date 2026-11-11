use mcrs_minecraft_bedrock_wire::{BeI32, BedrockDecode, BedrockEncode};

/// The first packet of a client: it asks for the compression settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0xc1)]
pub struct RequestNetworkSettings {
    /// The protocol version of the client.
    pub protocol_version: BeI32,
}
