use crate::login::GameProfile;
use crate::world::bus::{
    InboundConfirmMove, InboundPlayerDespawn, InboundPlayerSpawn, InboundRollbackMove,
    OutboundPlayerAttached, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
    PlayerInfoEntry,
};
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use crate::world::entity::player::chat::ChatPlugin;
use crate::world::entity::player::column_view::ColumnViewPlugin;
use crate::world::entity::player::digging::DiggingPlugin;
use crate::world::entity::player::game_mode::GameModePlugin;
use crate::world::entity::player::inventory::PlayerInventoryPlugin;
use crate::world::entity::player::movement::MovementPlugin;
use crate::world::entity::player::player_action::PlayerActionPlugin;
use crate::world::entity::{EntityBundle, MinecraftEntityType};
use crate::world::inventory::{ContainerSeqno, PlayerInventoryBundle};
use bevy_app::{FixedUpdate, Plugin, Update};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Commands, Query, ResMut, With};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::{PlayerChunkObserver, PlayerViewDistance};
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::entity::{Despawned, EntityNetworkAddEvent, InTransit};
use mcrs_engine::world::dimension::{Dimension, DimensionId, InDimension};
use mcrs_engine::session::{DimPlayerIndex, Owner, PlayerSession};
use crate::world::sub_app_builder::DimTypeIndex;
use mcrs_protocol::GameMode;
use movement::TeleportState;
use tracing::{debug, info};

pub mod ability;
pub mod attribute;
mod chat;
pub mod column_view;
pub mod digging;
mod game_mode;
mod inventory;
pub mod movement;
pub mod player_action;

/// Default game mode applied to joining players, read from `MCRS_DEFAULT_GAMEMODE`
/// (`survival`, `creative`, `adventure`, or `spectator`). Falls back to creative
/// when unset or unrecognized.
fn default_game_mode() -> GameMode {
    match std::env::var("MCRS_DEFAULT_GAMEMODE") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "survival" => GameMode::Survival,
            "creative" => GameMode::Creative,
            "adventure" => GameMode::Adventure,
            "spectator" => GameMode::Spectator,
            other => {
                tracing::warn!(
                    value = other,
                    "MCRS_DEFAULT_GAMEMODE unrecognized, defaulting to creative"
                );
                GameMode::Creative
            }
        },
        Err(_) => GameMode::Creative,
    }
}

/// Carries the host-anchor entity on the in-dim player entity. Inserted by
/// the per-dim spawn consumer so that subsequent per-dim systems can build
/// `PacketTarget::SinglePlayer(host_anchor)` without querying the host's
/// `PlayerIndex` or `ServerSideConnection`.
#[derive(bevy_ecs::component::Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAnchor(pub Entity);

/// Plugin for per-dimension worlds. Registers only the bus-driven and
/// simulation systems that are safe to run inside a DimWorld — no
/// `ServerSideConnection` or other host-only resource is accessed.
pub struct DimPlayerPlugin;

impl Plugin for DimPlayerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(DiggingPlugin);
        app.add_plugins(PlayerActionPlugin);
        app.add_plugins(MovementPlugin);
        app.add_plugins(ColumnViewPlugin);
        app.add_plugins(PlayerInventoryPlugin);
        app.add_plugins(ChatPlugin);
        app.add_plugins(GameModePlugin);
        app.add_systems(Update, consume_inbound_player_spawn);
        app.add_systems(Update, despawn_inbound_player);
        app.add_systems(FixedUpdate, (despawn_on_confirm, unhide_on_rollback));
        app.add_observer(network_add);
        app.add_observer(player_joined);
    }
}

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub teleport_state: TeleportState,
    pub view_distance: PlayerViewDistance,
    pub reposition: Reposition,
    pub abilities: ability::PlayerAbilitiesBundle,
    pub attributes: attribute::PlayerAttributesBundle,
    pub inventory: PlayerInventoryBundle,
    pub container_seqno: ContainerSeqno,
    pub game_mode: PlayerGameMode,
    pub op_level: PlayerOpLevel,
    pub chunk_subscription_set: crate::world::aoi::ChunkSubscriptionSet,
    pub tracked_by: crate::world::aoi::TrackedBy,
    pub marker: Player,
}

/// Per-dim system that materialises an in-dim player entity from an
/// `InboundPlayerSpawn` shuttled across the host→SubApp bus.
///
/// The connection stays host-resident. This system only creates the
/// simulation-side entity and signals the host to bind `in_dim_entity`
/// via `OutboundPlayerAttached`. `PlayerIndex` and `ServerSideConnection`
/// are host-resident and must NOT be accessed here.
fn consume_inbound_player_spawn(
    mut reader: MessageReader<InboundPlayerSpawn>,
    mut attached: MessageWriter<OutboundPlayerAttached>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    dims: Query<(Entity, &DimensionId, &DimTypeIndex), With<Dimension>>,
    mut commands: Commands,
    mut dim_index: ResMut<DimPlayerIndex>,
) {
    use std::sync::atomic::Ordering;
    for spawn in reader.read() {
        let Some((dim, dim_id, dim_type_index)) = dims.iter().next() else {
            continue;
        };
        let dim_name = dim_id.as_str().to_string();
        let dim_type_id = dim_type_index.0;
        let new_entity = commands
            .spawn((
                EntityBundle::new(InDimension(dim))
                    .with_uuid(spawn.snapshot.uuid)
                    .with_transform(
                        Transform::default()
                            .with_translation(spawn.snapshot.position),
                    ),
                PlayerBundle {
                    game_mode: PlayerGameMode(default_game_mode()),
                    teleport_state: TeleportState::after_login(),
                    ..Default::default()
                },
                PlayerChunkObserver::default(),
                HostAnchor(spawn.host_anchor),
                Owner(spawn.session),
                GameProfile {
                    id: spawn.snapshot.uuid,
                    username: spawn.snapshot.username.clone(),
                    properties: Vec::new(),
                },
            ))
            .id();
        dim_index.0.insert(spawn.session, new_entity);

        let host = spawn.host_anchor;
        let wire_id = new_entity.index_u32() as i32;
        let spawn_pos = spawn.snapshot.position;
        let center_x = (spawn_pos.x / 16.0).floor() as i32;
        let center_z = (spawn_pos.z / 16.0).floor() as i32;

        let dimensions = spawn.dimensions.clone();

        debug!(
            target: "mcrs_minecraft::player",
            player = wire_id,
            host_anchor = ?host,
            "emit_play_login: emitting play ClientboundLogin for newly-materialized in-dim entity"
        );

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::PlayerLogin {
                player_id: wire_id,
                hardcore: false,
                game_mode: default_game_mode(),
                dimension: dim_name,
                dimension_type_id: dim_type_id,
                dimensions,
                max_players: 100,
                chunk_radius: 12,
                simulation_distance: 12,
                reduced_debug_info: false,
                show_death_screen: false,
                do_limited_crafting: false,
                enforces_secure_chat: false,
            },
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        // The client derives the local player's game mode (and therefore
        // spectator noclip) from its own player-list entry, not the login
        // packet. Without this the client treats itself as non-spectator and
        // keeps block collisions even though login set the spectator mode.
        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::PlayerInfoUpdate {
                entries: vec![PlayerInfoEntry {
                    player_uuid: spawn.snapshot.uuid,
                    username: spawn.snapshot.username.clone(),
                    game_mode: default_game_mode(),
                    listed: true,
                }],
            },
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::SetChunkCacheCenter { x: center_x, z: center_z },
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::SetChunkCacheRadius { radius: 12 },
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::LevelChunksLoadStart,
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::PlayerLoginEntityEvent {
                entity_id: wire_id,
                entity_status: 24,
            },
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::PlayerPosition {
                teleport_id: TeleportState::LOGIN_TELEPORT_ID,
                position: spawn_pos,
            },
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        attached.write(OutboundPlayerAttached {
            host_anchor: spawn.host_anchor,
            new_in_dim_entity: new_entity,
        });
    }
}


/// Per-dim consumer that despawns the in-dim player entity when an
/// `InboundPlayerDespawn` arrives for its host anchor. Fires on both
/// disconnect and dimension transfer (the transfer pushes a despawn into the
/// dimension the player is leaving), so the departed dimension stops streaming
/// chunks toward that connection.
pub fn despawn_inbound_player(
    mut reader: MessageReader<InboundPlayerDespawn>,
    players: Query<(Entity, &HostAnchor), With<Player>>,
    mut commands: Commands,
    mut dim_index: ResMut<DimPlayerIndex>,
) {
    for msg in reader.read() {
        dim_index.0.remove(&msg.session);
        for (entity, anchor) in players.iter() {
            if anchor.0 == msg.host_anchor {
                commands.entity(entity).despawn();
            }
        }
    }
}

#[derive(EntityEvent)]
pub struct PlayerJoinEvent {
    #[event_target]
    pub player: Entity,
}

fn network_add(
    event: On<EntityNetworkAddEvent>,
    added_player: Query<(Entity, &GameProfile, &Transform), With<Player>>,
    viewer: Query<(&Reposition, &crate::world::player_index::HostAnchorRef), With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    use std::sync::atomic::Ordering;
    let Ok((entity, profile, transform)) = added_player.get(event.entity) else {
        return;
    };
    let Ok((reposition, host_anchor_ref)) = viewer.get(event.player) else {
        return;
    };

    let host_anchor = host_anchor_ref.0;
    packet_writer.write(OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(host_anchor),
        priority: PacketPriority::Normal,
        data: PacketPayload::PlayerEnteredView {
            entity_id: entity.index_u32() as i32,
            uuid: profile.id,
            kind: MinecraftEntityType::Player as i32,
            position: reposition.convert_dvec3(transform.translation),
            yaw: transform.rotation.y,
            pitch: transform.rotation.x,
        },
    session: PlayerSession(0),
    epoch: 0,
    });
    mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
        .fetch_add(1, Ordering::Relaxed);
}

fn player_joined(
    event: On<PlayerJoinEvent>,
    players: Query<(&GameProfile, &PlayerGameMode, &crate::world::player_index::HostAnchorRef), With<Player>>,
    positions: Query<&Transform, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    use std::sync::atomic::Ordering;
    let Ok((joined_player, _, _)) = players.get(event.player) else {
        return;
    };

    info!(
        "{} logged in with entity id {} at {}",
        joined_player.username,
        event.player,
        positions
            .get(event.player)
            .map(|pos| format!("{}", pos.translation))
            .unwrap_or_default()
    );

    let entries: Vec<PlayerInfoEntry> = players
        .iter()
        .map(|(profile, game_mode, _)| PlayerInfoEntry {
            player_uuid: profile.id,
            username: profile.username.clone(),
            game_mode: game_mode.0,
            listed: true,
        })
        .collect();

    // Broadcast player info to every connected player (including the joining player).
    for (_, _, host_anchor_ref) in players.iter() {
        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host_anchor_ref.0),
            priority: PacketPriority::Normal,
            data: PacketPayload::PlayerInfoUpdate {
                entries: entries.clone(),
            },
        session: PlayerSession(0),
        epoch: 0,
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);
    }
}

/// Source-dim system: when `ConfirmMove` arrives, find the in-transit entity
/// and despawn it — the entity has safely arrived at the target and is no longer
/// needed in the source dim.
pub fn despawn_on_confirm(
    mut reader: MessageReader<InboundConfirmMove>,
    in_transit: Query<(Entity, &InTransit)>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        for (entity, transit) in in_transit.iter() {
            if transit.move_id == msg.move_id {
                commands.entity(entity).remove::<InTransit>().insert(Despawned);
                break;
            }
        }
    }
}

/// Source-dim system: when `RollbackMove` arrives, remove `InTransit` from the
/// in-transit entity so it reappears at its original position — no despawn.
pub fn unhide_on_rollback(
    mut reader: MessageReader<InboundRollbackMove>,
    in_transit: Query<(Entity, &InTransit)>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        for (entity, transit) in in_transit.iter() {
            if transit.move_id == msg.move_id {
                commands.entity(entity).remove::<InTransit>();
                break;
            }
        }
    }
}

