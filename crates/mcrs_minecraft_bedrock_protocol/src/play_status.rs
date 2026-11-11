use mcrs_minecraft_bedrock_wire::{BeI32, BedrockDecode, BedrockEncode};

/// The result of a login, or the signal that the client can show the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x02)]
pub struct PlayStatus {
    /// The status.
    pub status: PlayStatusKind,
}

/// The status of [`PlayStatus`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = BeI32)]
#[non_exhaustive]
pub enum PlayStatusKind {
    /// The login is accepted.
    LoginSuccess = 0,
    /// The login fails: the client is older than the server.
    ClientOutdated = 1,
    /// The login fails: the server is older than the client.
    ServerOutdated = 2,
    /// The player spawns: the client can show the world.
    PlayerSpawn = 3,
    /// The login fails: the Education Edition tenant is not valid.
    InvalidTenant = 4,
    /// The login fails: an Education Edition client joins a server that is not Education Edition.
    EducationClientOnVanillaServer = 5,
    /// The login fails: a client that is not Education Edition joins an Education Edition server.
    VanillaClientOnEducationServer = 6,
    /// The login fails: the server is full for a sub-client.
    ServerFullForSubClient = 7,
    /// The login fails: an Editor client joins a server that is not an Editor server.
    EditorClientOnVanillaServer = 8,
    /// The login fails: a client that is not an Editor client joins an Editor server.
    VanillaClientOnEditorServer = 9,
}
