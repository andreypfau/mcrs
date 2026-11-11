use uuid::Uuid;

use crate::{BedrockUuidScheme, GameProfileValue, PlayerName, Xuid};

/// The external identity of a player. The UUID derives from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerIdentity {
    /// A Java player, or a Bedrock player with a linked Java account.
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
