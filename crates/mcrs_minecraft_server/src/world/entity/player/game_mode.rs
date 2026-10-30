use crate::world::entity::player::dimension_change::CarryOnDimensionChangeExt;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_engine::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine::bus::{
    OutboundPlayerPacket, PacketPayload, PacketTarget, PlayerInfoEntry, to_target,
};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_engine::session::HostAnchor;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::GameEventKind;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundGameEvent, ClientboundPlayerAbilities,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChangeGameMode, ServerboundPlayerAbilities,
};

/// The game mode that the player list shows for a player, which the login and a dimension
/// change leave as it was told.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListedGameMode(pub GameMode);

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_change_game_mode)
            .add_observer(handle_player_abilities)
            .carry_on_dimension_change::<PlayerGameMode>()
            .carry_on_dimension_change::<PlayerFlying>()
            .carry_on_dimension_change::<ListedGameMode>();
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
    if mcrs_minecraft_command_engine::level_of(*op_level) < CommandLevel::Gamemasters {
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
        flying.set_if_neq(PlayerFlying(
            pkt.flying && abilities(mode.0, PlayerFlying(true)).may_fly,
        ));
    }
}

// chisle: the flying and walking speeds are the defaults, because the saved speeds of a player
// are not read yet; reading them with the rest of the player data lifts it.
const FLYING_SPEED: f32 = 0.05;
const WALKING_SPEED: f32 = 0.1;

/// The abilities of a player in `mode` that flew as `flying` before.
pub(crate) fn abilities(mode: GameMode, flying: PlayerFlying) -> ClientboundPlayerAbilities {
    let (invulnerable, flying, may_fly, instant_build) = match mode {
        GameMode::Survival | GameMode::Adventure => (false, false, false, false),
        GameMode::Creative => (true, flying.0, true, true),
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
/// of the server the new entry of its list. A mode that the login or a dimension
/// change already told is not sent again.
// chisle: a player that flies near the ground keeps its flight on a change to creative, since
// the server has no collision query for it yet; a ground test over the block world lifts it.
pub(crate) fn send_game_mode(
    mut players: Query<
        (
            &PlayerGameMode,
            &mut ListedGameMode,
            &mut PlayerFlying,
            &GameProfile,
            &HostAnchor,
        ),
        Changed<PlayerGameMode>,
    >,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    let mut entries = Vec::new();
    for (mode, mut listed, mut flying, profile, &HostAnchor(host)) in &mut players {
        if listed.0 == mode.0 {
            continue;
        }
        listed.0 = mode.0;
        let abilities = abilities(mode.0, *flying);
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
