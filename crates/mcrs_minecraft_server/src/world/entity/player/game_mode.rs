use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketTarget, PlayerInfoEntry, to_target,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundGameEvent, ClientboundPlayerAbilities,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChangeGameMode, ServerboundPlayerAbilities,
};
use mcrs_minecraft_protocol::{GameEventKind, GameMode};

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_change_game_mode)
            .add_observer(handle_player_abilities);
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
    if crate::command::level_of(*op_level) < CommandLevel::Gamemasters {
        tracing::warn!(
            "player {:?} tried to change game mode to {:?} without permission",
            event.entity,
            pkt.mode
        );
        return;
    }
    current_mode.set_if_neq(PlayerGameMode(pkt.mode));
}

fn handle_player_abilities(
    event: On<Received<ServerboundPlayerAbilities>>,
    mut players: Query<(&PlayerGameMode, &mut PlayerFlying), With<HostAnchor>>,
) {
    let Some(pkt) = event.packet() else {
        return;
    };
    if let Ok((mode, mut flying)) = players.get_mut(event.entity) {
        flying.set_if_neq(PlayerFlying(pkt.flying && abilities(mode.0, true).may_fly));
    }
}

// chisle: the flying and walking speeds are the defaults, because the saved speeds of a player
// are not read yet; reading them with the rest of the player data lifts it.
const FLYING_SPEED: f32 = 0.05;
const WALKING_SPEED: f32 = 0.1;

/// The abilities of a player in `mode` that flew as `flying` before (CMD-GM-007).
pub(crate) fn abilities(mode: GameMode, flying: bool) -> ClientboundPlayerAbilities {
    let (invulnerable, flying, may_fly, instant_build) = match mode {
        GameMode::Survival | GameMode::Adventure => (false, false, false, false),
        GameMode::Creative => (true, flying, true, true),
        GameMode::Spectator => (true, true, true, false),
    };
    ClientboundPlayerAbilities {
        invulnerable,
        flying,
        may_fly,
        instant_build,
        flying_speed: FLYING_SPEED,
        walking_speed: WALKING_SPEED,
    }
}

/// Gives a player the abilities of its new game mode and tells it both, and tells every player
/// of the server the new entry of its list (CMD-GM-003). A player that spawns learns its mode
/// from the login instead.
// chisle: a player that flies near the ground keeps its flight on a change to creative, since
// the server has no collision query for it yet; a ground test over the block world lifts it.
pub(crate) fn send_game_mode(
    mut players: Query<
        (
            Ref<PlayerGameMode>,
            &mut PlayerFlying,
            &GameProfile,
            &HostAnchor,
        ),
        Changed<PlayerGameMode>,
    >,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let mut entries = Vec::new();
    for (mode, mut flying, profile, &HostAnchor(host)) in &mut players {
        if mode.is_added() {
            continue;
        }
        let abilities = abilities(mode.0, flying.0);
        flying.set_if_neq(PlayerFlying(abilities.flying));
        writer.write(
            to(
                host,
                PacketPayload::GameEvent(ClientboundGameEvent {
                    game_event: GameEventKind::ChangeGameMode(mode.0),
                }),
            )
            .critical(),
        );
        writer.write(to(host, PacketPayload::PlayerAbilities(abilities)).critical());
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
    writer.write(to_target(
        PacketTarget::AllPlayers,
        PacketPayload::PlayerInfoUpdate { entries },
    ));
}
