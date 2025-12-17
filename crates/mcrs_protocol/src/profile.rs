use crate::{Bounded, Decode, Encode, GameMode};
use base64::prelude::*;
use bitfield_struct::bitfield;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use url::Url;
use uuid::Uuid;
use valence_text::Text;

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct GameProfile<'a> {
    pub id: Uuid,
    pub username: Bounded<&'a str, 16>,
    pub properties: Cow<'a, [Property<&'a str>]>,
}

/// A property from the game profile.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct Property<S = String> {
    pub name: S,
    pub value: S,
    pub signature: Option<S>,
}

/// Contains URLs to the skin and cape of a player.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerTextures {
    /// URL to the player's skin texture.
    pub skin: Url,
    /// URL to the player's cape texture. May be absent if the player does not
    /// have a cape.
    pub cape: Option<Url>,
}

impl PlayerTextures {
    /// Constructs player textures from the "textures" property of the game
    /// profile.
    ///
    /// "textures" is a base64 string of JSON data.
    pub fn try_from_textures(textures: &str) -> anyhow::Result<Self> {
        #[derive(Debug, Deserialize)]
        struct Textures {
            textures: PlayerTexturesPayload,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "UPPERCASE")]
        struct PlayerTexturesPayload {
            skin: TextureUrl,
            #[serde(default)]
            cape: Option<TextureUrl>,
        }

        #[derive(Debug, Deserialize)]
        struct TextureUrl {
            url: Url,
        }

        let decoded = BASE64_STANDARD.decode(textures.as_bytes())?;

        let Textures { textures } = serde_json::from_slice(&decoded)?;

        Ok(Self {
            skin: textures.skin.url,
            cape: textures.cape.map(|t| t.url),
        })
    }
}

#[bitfield(u8)]
pub struct PlayerListActions {
    pub add_player: bool,
    pub initialize_chat: bool,
    pub update_game_mode: bool,
    pub update_listed: bool,
    pub update_latency: bool,
    pub update_display_name: bool,
    #[bits(2)]
    _pad: u8,
}

#[derive(Clone, Default, Debug)]
pub struct PlayerListEntry<'a> {
    pub player_uuid: Uuid,
    pub username: &'a str,
    pub properties: Cow<'a, [Property]>,
    pub chat_data: Option<ChatData<'a>>,
    pub listed: bool,
    pub ping: i32,
    pub game_mode: GameMode,
    pub display_name: Option<Cow<'a, Text>>,
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct ChatData<'a> {
    pub session_id: Uuid,
    /// Unix timestamp in milliseconds.
    pub key_expiry_time: i64,
    pub public_key: &'a [u8],
    pub public_key_signature: &'a [u8],
}
