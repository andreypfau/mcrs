use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

/// The answer of a client to the pack offer of the server.
#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x08)]
pub struct ResourcePackClientResponse<'a> {
    /// The answer.
    pub response: ResourcePackResponse,
    /// The pack references (`<pack id>_<version>`) that the client asks for; empty unless
    /// the response is [`ResourcePackResponse::SendPacks`].
    #[bedrock(count = u16)]
    pub pack_references: Vec<&'a str>,
}

/// The answer of [`ResourcePackClientResponse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = u8)]
#[non_exhaustive]
pub enum ResourcePackResponse {
    /// The player does not accept the packs.
    Refused = 1,
    /// The client asks for the packs of the list.
    SendPacks = 2,
    /// The client has each pack of `ResourcePacksInfo`.
    HaveAllPacks = 3,
    /// The client applied the stack.
    Completed = 4,
}
