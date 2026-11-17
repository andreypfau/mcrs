use mcrs_minecraft_bedrock_wire::{BeI32, BedrockDecode, BedrockEncode};

use crate::connection_request::ConnectionRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x01)]
pub struct Login<'a> {
    pub protocol_version: BeI32,
    pub connection_request: ConnectionRequest<'a>,
}
