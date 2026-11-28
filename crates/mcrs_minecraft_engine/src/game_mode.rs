use crate::ability::{PlayerFlying, PlayerOpLevel};
use crate::bus::{OutboundPlayerPacket, PacketPayload, to};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::GameEventKind;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundGameEvent, ClientboundPlayerAbilities};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundChangeGameMode, ServerboundPlayerAbilities,
};

#[derive(
    Component, Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct ListedGameMode(pub GameMode);

pub struct GameModePlugin;

impl Plugin for GameModePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(handle_change_game_mode)
            .add_observer(handle_player_abilities);
    }
}

fn handle_change_game_mode(
    event: On<Received<ServerboundChangeGameMode>>,
    mut players: Query<(&PlayerOpLevel, &mut PlayerGameMode), With<Owner>>,
) {
    let Some(pkt) = event.packet() else {
        return;
    };
    let Ok((op_level, mut current_mode)) = players.get_mut(event.entity) else {
        return;
    };
    if op_level.clamped() < PlayerOpLevel::GAMEMASTERS {
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
    mut players: Query<(&PlayerGameMode, &mut PlayerFlying), With<Owner>>,
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

const FLYING_SPEED: f32 = 0.05;
const WALKING_SPEED: f32 = 0.1;

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

pub(crate) fn send_game_mode(
    mut players: Populated<
        (
            &PlayerGameMode,
            &mut ListedGameMode,
            &mut PlayerFlying,
            &Owner,
        ),
        Changed<PlayerGameMode>,
    >,
    mut writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (mode, listed, mut flying, &Owner(session)) in &mut players {
        if !listed
            .map_unchanged(|listed| &mut listed.0)
            .set_if_neq(mode.0)
        {
            continue;
        }
        let abilities = abilities(mode.0, *flying);
        flying.set_if_neq(PlayerFlying(abilities.flying));
        writer.write(to(
            session,
            PacketPayload::GameEvent(ClientboundGameEvent {
                game_event: GameEventKind::ChangeGameMode(mode.0),
            }),
        ));
        writer.write(to(session, PacketPayload::PlayerAbilities(abilities)));
    }
}
