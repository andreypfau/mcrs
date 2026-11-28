use uuid::Uuid;

use crate::{BedrockUuidScheme, GameProfileValue, PlayerName, Xuid};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerIdentity {
    Java(GameProfileValue),
    Bedrock(BedrockIdentity),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BedrockIdentity {
    pub xuid: Xuid,
    pub gamertag: String,
    pub name: PlayerName,
    pub scheme: BedrockUuidScheme,
}

impl PlayerIdentity {
    #[must_use]
    pub fn id(&self) -> Uuid {
        match self {
            Self::Java(profile) => profile.id,
            Self::Bedrock(bedrock) => bedrock.scheme.to_uuid(bedrock.xuid),
        }
    }

    #[must_use]
    pub fn name(&self) -> &PlayerName {
        match self {
            Self::Java(profile) => &profile.name,
            Self::Bedrock(bedrock) => &bedrock.name,
        }
    }
}
