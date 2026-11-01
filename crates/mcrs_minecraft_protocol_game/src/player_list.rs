use bitfield_struct::bitfield;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_profile::Property;
use mcrs_minecraft_text::RawText;
use mcrs_minecraft_wire::{Decode, Encode};
use std::borrow::Cow;
use uuid::Uuid;

#[bitfield(u8)]
pub struct PlayerListActions {
    pub add_player: bool,
    pub initialize_chat: bool,
    pub update_game_mode: bool,
    pub update_listed: bool,
    pub update_latency: bool,
    pub update_display_name: bool,
    pub update_list_order: bool,
    pub update_hat: bool,
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
    pub display_name: Option<RawText<'a>>,
    pub list_order: i32,
    pub show_hat: bool,
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct ChatData<'a> {
    pub session_id: Uuid,
    /// Unix timestamp in milliseconds.
    pub key_expiry_time: i64,
    pub public_key: &'a [u8],
    pub public_key_signature: &'a [u8],
}
