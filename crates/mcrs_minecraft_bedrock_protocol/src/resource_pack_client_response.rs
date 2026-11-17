use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode};

#[derive(Clone, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x08)]
pub struct ResourcePackClientResponse<'a> {
    pub response: ResourcePackResponse,
    #[bedrock(count = u16)]
    pub pack_references: Vec<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = u8)]
#[non_exhaustive]
pub enum ResourcePackResponse {
    Refused = 1,
    SendPacks = 2,
    HaveAllPacks = 3,
    Completed = 4,
}
