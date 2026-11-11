use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32};

/// The movement settings of [`StartGame`](crate::StartGame).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct MovementSettings {
    /// The side that owns the movement; it selects the movement packet of the client.
    pub authority: MovementAuthority,
    /// The rewind history in client ticks; used only with [`MovementAuthority::ServerWithRewind`].
    pub rewind_history_size: VarI32,
    /// Whether the server owns block breaking.
    pub server_authoritative_block_breaking: bool,
}

/// The movement authority of [`MovementSettings`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum MovementAuthority {
    /// The client sends `MovePlayer`.
    Client = 0,
    /// The client sends `PlayerAuthInput` once for each client tick.
    Server = 1,
    /// As [`MovementAuthority::Server`], and the server can rewind the client.
    ServerWithRewind = 2,
}
