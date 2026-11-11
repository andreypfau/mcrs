use mcrs_minecraft_bedrock_wire::{BeI32, BedrockDecode, BedrockEncode};

use crate::connection_request::ConnectionRequest;

/// The login of a client, sent after the switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x01)]
pub struct Login<'a> {
    /// The protocol version of the client.
    pub protocol_version: BeI32,
    /// The identity chain and the client data.
    pub connection_request: ConnectionRequest<'a>,
}
