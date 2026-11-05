use crate::encode::record;
use bytes::Bytes;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_profile::Property;
use mcrs_minecraft_protocol::Result;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundPlayerInfoRemove, ClientboundPlayerInfoUpdate,
};
use mcrs_minecraft_protocol_game::player_list::{PlayerListActions, PlayerListEntry};
use std::borrow::Cow;
use uuid::Uuid;

/// The identity of a player as the player list shows it.
#[derive(Clone, Copy, Debug)]
pub struct Profile<'a> {
    pub id: Uuid,
    pub username: &'a str,
    pub properties: &'a [Property],
}

/// The packet that adds the entry of `profile`. It does not set the listing, so a new entry is
/// not listed.
pub(crate) fn profile_entry(profile: &Profile<'_>, game_mode: GameMode) -> Result<Bytes> {
    record(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new()
            .with_add_player(true)
            .with_update_game_mode(true),
        entries: Cow::Borrowed(&[PlayerListEntry {
            player_uuid: profile.id,
            username: profile.username,
            properties: Cow::Borrowed(profile.properties),
            game_mode,
            ..Default::default()
        }]),
    })
}

/// The packet that tells the viewers of an entry the new game mode of its player.
pub fn game_mode_packet(uuid: Uuid, game_mode: GameMode) -> Result<Bytes> {
    record(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_game_mode(true),
        entries: Cow::Borrowed(&[PlayerListEntry {
            player_uuid: uuid,
            game_mode,
            ..Default::default()
        }]),
    })
}

/// The packet that lists `entries` with their `ping`.
pub(crate) fn listed_packet(entries: Vec<PlayerListEntry<'static>>) -> Result<Bytes> {
    record(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new()
            .with_update_listed(true)
            .with_update_latency(true),
        entries: Cow::Owned(entries),
    })
}

pub(crate) fn unlisted_packet(players: &[Uuid]) -> Result<Bytes> {
    let entries: Vec<PlayerListEntry<'static>> = players
        .iter()
        .map(|&player_uuid| PlayerListEntry {
            player_uuid,
            ..Default::default()
        })
        .collect();
    record(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_listed(true),
        entries: Cow::Owned(entries),
    })
}

/// The packet that gives the `ping` of `entries`.
pub(crate) fn latency_packet(entries: Vec<PlayerListEntry<'static>>) -> Result<Bytes> {
    record(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_latency(true),
        entries: Cow::Owned(entries),
    })
}

pub(crate) fn remove_packet(players: Vec<Uuid>) -> Result<Bytes> {
    record(&ClientboundPlayerInfoRemove { players })
}
