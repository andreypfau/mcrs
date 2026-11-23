use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::{Has, With};
use bevy_ecs::system::{Commands, Query, Res};
use mcrs_minecraft_damage::{HitRecord, Vitals};
use mcrs_minecraft_engine::HiddenPlayer;
use mcrs_minecraft_engine::bus::{self, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_engine::coordinator::clock::GameTime;
use mcrs_minecraft_engine::coordinator::running_dimensions::{RunningDimension, RunningDimensions};
use mcrs_minecraft_engine::dimension_change::Arrival;
use mcrs_minecraft_engine::player_state::PlayerFileState;
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_engine::{KeptPlayerData, max_health, respawn_packet};
use mcrs_minecraft_engine_damage::RespawnRequest;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_protocol_game::GameEventKind;
use mcrs_minecraft_protocol_game::clientbound::ClientboundGameEvent;

use crate::send_health::SentHealth;

#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub(crate) struct HiddenForRespawn;

type RespawnedPlayer<'a> = (
    &'a mut Vitals,
    &'a mut HitRecord,
    &'a mut PlayerFileState,
    &'a mut Transform,
    &'a mut TeleportState,
    &'a PlayerGameMode,
    &'a Owner,
    Option<&'a AttributeInstances>,
    Has<HiddenPlayer>,
);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::marker_toggle,
        reason = "the marker flips only once per death and once per respawn, and the player tile publisher filters on it"
    )
)]
pub(crate) fn respawn_players(
    mut requests: MessageReader<RespawnRequest>,
    mut players: Query<RespawnedPlayer, With<Player>>,
    own: Option<Res<DimensionLabel>>,
    running: Option<Res<RunningDimensions>>,
    time: Res<GameTime>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut changes: MessageWriter<ChangeDimensionRequest>,
    mut commands: Commands,
) {
    for request in requests.read() {
        let Ok((
            mut vitals,
            mut hits,
            mut file_state,
            mut transform,
            mut teleport,
            game_mode,
            &Owner(session),
            attributes,
            hidden,
        )) = players.get_mut(request.player)
        else {
            continue;
        };
        if vitals.is_alive() {
            continue;
        }
        let Some(dimension) = running.as_deref().and_then(|running| {
            running
                .iter()
                .find(|found| found.label == request.dimension)
        }) else {
            tracing::error!(
                player = ?request.player,
                dimension = ?request.dimension,
                "a player does not respawn in a dimension that does not run"
            );
            continue;
        };
        vitals.set_if_neq(Vitals::new(max_health(attributes)));
        hits.set_if_neq(HitRecord::default());
        reset_file_state(&mut file_state);
        if own.as_deref().map(|own| own.0) != Some(request.dimension) {
            changes.write(ChangeDimensionRequest {
                player: request.player,
                to: dimension.key.clone(),
                transform: request.transform,
            });
            continue;
        }
        let arrival = Arrival::absolute(request.transform);
        let teleport_id = teleport.sent(arrival.transform, *time);
        *transform = arrival.transform;
        packets.write_batch(enter_packets(
            session,
            dimension,
            game_mode,
            &arrival,
            teleport_id,
        ));
        let mut player = commands.entity(request.player);
        player.remove::<SentHealth>();
        if hidden {
            player.remove::<HiddenPlayer>();
        } else {
            player.insert((HiddenPlayer, HiddenForRespawn));
        }
    }
}

fn enter_packets(
    session: PlayerSession,
    dimension: &RunningDimension,
    game_mode: &PlayerGameMode,
    arrival: &Arrival,
    teleport_id: i32,
) -> [OutboundPlayerPacket; 3] {
    [
        bus::to(
            session,
            respawn_packet(
                dimension.dimension_type,
                dimension.key.clone(),
                game_mode.0,
                KeptPlayerData::Nothing,
            ),
        ),
        bus::to(
            session,
            PacketPayload::PlayerPosition(arrival.position_packet(teleport_id)),
        ),
        bus::to(
            session,
            PacketPayload::GameEvent(ClientboundGameEvent {
                game_event: GameEventKind::LevelChunksLoadStart,
            }),
        ),
    ]
}

fn reset_file_state(state: &mut PlayerFileState) {
    let fresh = PlayerFileState::default();
    state.food_level = fresh.food_level;
    state.food_saturation_level = fresh.food_saturation_level;
    state.food_exhaustion_level = fresh.food_exhaustion_level;
    state.fire = fresh.fire;
    state.ticks_frozen = fresh.ticks_frozen;
    state.air = fresh.air;
    state.fall_distance = fresh.fall_distance;
    state.portal_cooldown = fresh.portal_cooldown;
}

pub(crate) fn show_respawned_players(
    players: Query<Entity, With<HiddenForRespawn>>,
    mut commands: Commands,
) {
    for player in &players {
        commands
            .entity(player)
            .remove::<(HiddenPlayer, HiddenForRespawn)>();
    }
}
