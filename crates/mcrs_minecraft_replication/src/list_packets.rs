use crate::encode::encode;
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

/// The round trip to a player in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EntryLatency {
    pub player_uuid: Uuid,
    pub latency: i32,
}

/// The packet that adds the entry of `profile`. It does not set the listing, so a new entry is
/// not listed.
pub(crate) fn profile_entry(profile: &Profile<'_>, game_mode: GameMode) -> Result<Bytes> {
    encode(&ClientboundPlayerInfoUpdate {
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
    encode(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_game_mode(true),
        entries: Cow::Borrowed(&[PlayerListEntry {
            player_uuid: uuid,
            game_mode,
            ..Default::default()
        }]),
    })
}

fn latency_entries(entries: &[EntryLatency], listed: bool) -> Vec<PlayerListEntry<'static>> {
    entries
        .iter()
        .map(|entry| PlayerListEntry {
            player_uuid: entry.player_uuid,
            listed,
            ping: entry.latency,
            ..Default::default()
        })
        .collect()
}

pub(crate) fn listed_packet(entries: &[EntryLatency]) -> Result<Bytes> {
    encode(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new()
            .with_update_listed(true)
            .with_update_latency(true),
        entries: Cow::Owned(latency_entries(entries, true)),
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
    encode(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_listed(true),
        entries: Cow::Owned(entries),
    })
}

pub(crate) fn latency_packet(entries: &[EntryLatency]) -> Result<Bytes> {
    encode(&ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new().with_update_latency(true),
        entries: Cow::Owned(latency_entries(entries, false)),
    })
}

pub(crate) fn remove_packet(players: Vec<Uuid>) -> Result<Bytes> {
    encode(&ClientboundPlayerInfoRemove { players })
}
