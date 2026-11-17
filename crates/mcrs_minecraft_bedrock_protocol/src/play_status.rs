use mcrs_minecraft_bedrock_wire::{BeI32, BedrockDecode, BedrockEncode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x02)]
pub struct PlayStatus {
    pub status: PlayStatusKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = BeI32)]
#[non_exhaustive]
pub enum PlayStatusKind {
    LoginSuccess = 0,
    ClientOutdated = 1,
    ServerOutdated = 2,
    PlayerSpawn = 3,
    InvalidTenant = 4,
    EducationClientOnVanillaServer = 5,
    VanillaClientOnEducationServer = 6,
    ServerFullForSubClient = 7,
    EditorClientOnVanillaServer = 8,
    VanillaClientOnEditorServer = 9,
}
