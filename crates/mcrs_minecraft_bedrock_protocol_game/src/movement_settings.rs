use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct MovementSettings {
    pub authority: MovementAuthority,
    pub rewind_history_size: VarI32,
    pub server_authoritative_block_breaking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum MovementAuthority {
    Client = 0,
    Server = 1,
    ServerWithRewind = 2,
}
