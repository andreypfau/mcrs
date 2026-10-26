use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget, PlayerInfoEntry,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::GameEventKind;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChangeGameMode;

const REQUIRED_OP_LEVEL: u8 = 2;

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_change_game_mode);
    }
}

fn handle_change_game_mode(
    event: On<Received<ServerboundChangeGameMode>>,
    mut players: Query<(&PlayerOpLevel, &mut PlayerGameMode), With<HostAnchor>>,
) {
    let Some(pkt) = event.packet() else {
        return;
    };
    let Ok((op_level, mut current_mode)) = players.get_mut(event.entity) else {
        return;
    };
    if op_level.clamped() < REQUIRED_OP_LEVEL {
        tracing::warn!(
            "player {:?} tried to change game mode to {:?} without permission",
            event.entity,
            pkt.mode
        );
        return;
    }
    current_mode.set_if_neq(PlayerGameMode(pkt.mode));
}

/// Tells a player its new game mode, and every player of the server the new entry of its list.
/// A player that spawns learns its mode from the login instead.
pub(crate) fn send_game_mode(
    players: Query<(Ref<PlayerGameMode>, &GameProfile, &HostAnchor), Changed<PlayerGameMode>>,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let mut entries = Vec::new();
    for (mode, profile, &HostAnchor(host)) in &players {
        if mode.is_added() {
            continue;
        }
        writer.write(
            to(
                host,
                PacketPayload::GameEvent(ClientboundGameEvent {
                    game_event: GameEventKind::ChangeGameMode(mode.0),
                }),
            )
            .critical(),
        );
        entries.push(PlayerInfoEntry {
            player_uuid: profile.id,
            username: profile.username.clone(),
            game_mode: mode.0,
            listed: true,
        });
    }
    if entries.is_empty() {
        return;
    }
    writer.write(OutboundPlayerPacket {
        target: PacketTarget::AllPlayers,
        priority: PacketPriority::Normal,
        data: PacketPayload::PlayerInfoUpdate { entries },
        session: PlayerSession(0),
        epoch: 0,
    });
}
