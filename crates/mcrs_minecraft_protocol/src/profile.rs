pub use crate::item::component::profile::{PlayerTextures, Property};
use crate::text::Text;
use crate::{Bounded, Decode, Encode, GameMode};
use bitfield_struct::bitfield;
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct GameProfile<'a> {
    pub id: Uuid,
    pub username: Bounded<&'a str, 16>,
    pub properties: Cow<'a, [Property<&'a str>]>,
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
