//! The chat messages about a player that joins or leaves, which go only to the players near it.

use crate::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, to_target};
use crate::entity_tracker::EntityTracker;
use crate::session::GameProfile;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Commands, Query, Res, With};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_protocol_game::clientbound::ClientboundSystemChatPacket;
use mcrs_minecraft_text::{Color, IntoText, RawText, Text};
use smallvec::SmallVec;

/// Marks a player that logged in and that its neighbours have not heard of yet. A player that
/// comes from another dimension does not carry it.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AnnounceJoin;

/// Tells each player that joined, and the players that have it in their visible set, that it
/// joined.
pub fn announce_joins(
    tracker: Res<EntityTracker>,
    joined: Query<(Entity, &GameProfile, &Owner), With<AnnounceJoin>>,
    mut commands: Commands,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (player, profile, &Owner(session)) in &joined {
        if tracker.get(player).is_none() {
            continue;
        }
        commands.entity(player).remove::<AnnounceJoin>();
        let mut sessions = tracker.viewer_sessions(player);
        sessions.push(session);
        if let Some(packet) = presence_packet(sessions, "multiplayer.player.joined", profile) {
            writer.write(packet);
        }
    }
}

/// The message that `player` left, for the players that have it in their visible set.
pub fn leave_message(
    tracker: &EntityTracker,
    player: Entity,
    profile: &GameProfile,
) -> Option<OutboundPlayerPacket> {
    let sessions = tracker.viewer_sessions(player);
    if sessions.is_empty() {
        return None;
    }
    presence_packet(sessions, "multiplayer.player.left", profile)
}

fn presence_packet(
    sessions: SmallVec<[PlayerSession; 8]>,
    key: &'static str,
    profile: &GameProfile,
) -> Option<OutboundPlayerPacket> {
    let text: Text =
        Text::translate(key, vec![profile.username.clone().into_text()]).color(Color::YELLOW);
    let content = RawText::from_text(&text).ok()?;
    Some(to_target(
        PacketTarget::PlayerSet(sessions),
        PacketPayload::SystemChat(ClientboundSystemChatPacket {
            content,
            overlay: false,
        }),
    ))
}
