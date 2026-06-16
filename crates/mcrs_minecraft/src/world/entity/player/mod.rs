use crate::client_info::ClientViewDistance;
use crate::configuration::LoadedWorldPreset;
use crate::login::GameProfile;
use crate::world::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerPacket,
    PacketPayload, PacketPriority, PacketTarget, PlayerInfoEntry,
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
use crate::world::inventory::{ContainerSeqno, PlayerInventoryBundle, PlayerInventoryQuery};
use crate::world::item::minecraft::DIAMOND_PICKAXE;
use crate::world::item::{ItemCommands, ItemStack};
use bevy_app::{FixedUpdate, Plugin, PostUpdate};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Changed, Commands, Has, Query, RemovedComponents, Res, With};
use bevy_ecs::query::Added;
use bevy_math::DVec3;
use derive_more::{Deref, DerefMut};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::{PlayerChunkObserver, PlayerViewDistance};
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::entity::{Despawned, EntityNetworkAddEvent};
use mcrs_engine::world::dimension::{Dimension, DimensionId, InDimension};
use crate::world::sub_app_builder::DimTypeIndex;
use mcrs_network::{ConnectionState, InGameConnectionState, ServerSideConnection};
use mcrs_protocol::entity::player::PlayerSpawnInfo;
use mcrs_protocol::item::ComponentPatch;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundContainerSetContent, ClientboundDisconnect, ClientboundEntityEvent,
    ClientboundGameEvent, ClientboundLogin, ClientboundPlayerPosition,
};
use mcrs_protocol::{GameEventKind, GameMode, Look, Slot, Text, VarInt, WritePacket};
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

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(DiggingPlugin);
        app.add_plugins(PlayerActionPlugin);
        app.add_plugins(MovementPlugin);
        app.add_plugins(ColumnViewPlugin);
        app.add_plugins(PlayerInventoryPlugin);
        app.add_plugins(ChatPlugin);
        app.add_plugins(GameModePlugin);
        app.add_systems(bevy_app::Update, spawn_player);
        app.add_systems(bevy_app::Update, consume_inbound_player_spawn);
        app.add_systems(bevy_app::Update, despawn_inbound_player);
        app.add_systems(FixedUpdate, (disconnect_player, added_inventory, resync_player));
        app.add_systems(PostUpdate, despawn_disconnected_clients);
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

/// Marker component that triggers a full re-sync of player state to the client.
/// Added during reconfiguration to re-send position, inventory, etc.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ResyncPlayer;

#[derive(Clone, Debug, PartialEq, Component, Deref, DerefMut)]
pub struct DisconnectReason(pub Text);

fn spawn_player(
    world_preset: Res<LoadedWorldPreset>,
    dimensions: Query<(Entity, &DimensionId), With<Dimension>>,
    mut query: Query<
        (
            Entity,
            &ClientViewDistance,
            &ConnectionState,
            &GameProfile,
            &mut ServerSideConnection,
            Has<Player>,
            Option<&PlayerGameMode>,
            Option<&PlayerOpLevel>,
        ),
        Changed<ConnectionState>,
    >,
    mut commands: Commands,
) {
    // Wait for the world preset to be loaded via Bevy assets
    if !world_preset.is_loaded {
        return;
    }

    query
        .iter_mut()
        .for_each(|(entity, distance, con_state, profile, mut con, is_reconfiguration, existing_game_mode, existing_op_level)| {
            if *con_state != ConnectionState::Game {
                return;
            }
            let game_mode = existing_game_mode
                .map(|gm| gm.0)
                .unwrap_or_else(default_game_mode);
            let op_level = existing_op_level
                .copied()
                .unwrap_or(PlayerOpLevel(PlayerOpLevel::MAX));
            // Find dimension by first DimensionId in preset order
            let dim = if let Some((first_dim_key, _)) = world_preset.dimensions.first() {
                // Look for the dimension entity matching the first preset dimension
                dimensions
                    .iter()
                    .find(|(_, dim_id)| dim_id.as_str() == first_dim_key.as_str())
                    .map(|(entity, _)| entity)
            } else {
                // Fallback: use any available dimension if preset is empty
                dimensions.iter().next().map(|(entity, _)| entity)
            };
            let Some(dim) = dim else {
                tracing::warn!("No dimension found! Can't spawn player yet - dimensions may still be loading.");
                return;
            };

            if is_reconfiguration {
                // Reconfiguration: player already exists, re-send login with updated dimension types
                info!("Reconfiguring player {:?} with updated registries", entity);

                con.write_packet(&ClientboundLogin {
                    player_id: entity.index_u32() as i32,
                    hardcore: false,
                    dimensions: world_preset
                        .dimensions
                        .iter()
                        .map(|(dim_key, _)| dim_key.clone().into())
                        .collect(),
                    max_players: VarInt(100),
                    chunk_radius: VarInt(12),
                    simulation_distance: VarInt(12),
                    reduced_debug_info: false,
                    show_death_screen: false,
                    do_limited_crafting: false,
                    player_spawn_info: PlayerSpawnInfo {
                        game_mode,
                        ..Default::default()
                    },
                    enforces_secure_chat: false,
                });
                con.write_packet(&ClientboundGameEvent {
                    game_event: GameEventKind::LevelChunksLoadStart,
                });
                con.write_packet(&ClientboundEntityEvent {
                    entity_id: entity.index_u32() as i32,
                    entity_status: op_level.entity_status(),
                });

                // Insert marker to trigger re-sync of position, inventory, chunks
                commands.entity(entity).insert((
                    ResyncPlayer,
                    PlayerChunkObserver::default(),
                ));
            } else {
                // Initial spawn: full login flow
                con.write_packet(&ClientboundLogin {
                    player_id: entity.index_u32() as i32,
                    hardcore: false,
                    dimensions: world_preset
                        .dimensions
                        .iter()
                        .map(|(dim_key, _)| dim_key.clone().into())
                        .collect(),
                    max_players: VarInt(100),
                    chunk_radius: VarInt(12),
                    simulation_distance: VarInt(12),
                    reduced_debug_info: false,
                    show_death_screen: false,
                    do_limited_crafting: false,
                    player_spawn_info: PlayerSpawnInfo {
                        game_mode,
                        ..Default::default()
                    },
                    enforces_secure_chat: false,
                });
                con.write_packet(&ClientboundGameEvent {
                    game_event: GameEventKind::LevelChunksLoadStart,
                });
                con.write_packet(&ClientboundEntityEvent {
                    entity_id: entity.index_u32() as i32,
                    entity_status: op_level.entity_status(),
                });
                let pos = DVec3::new(0.0, 64.0, 0.0);

                let pickaxe = commands.spawn_item_stack(&DIAMOND_PICKAXE, 1);
                let mut inventory = PlayerInventoryBundle::default();
                inventory.hotbar.slots[0] = Some(pickaxe);

                commands.entity(entity).insert((
                    PlayerChunkObserver {
                        ..Default::default()
                    },
                    EntityBundle::new(InDimension(dim))
                        .with_uuid(profile.id)
                        .with_transform(Transform::default().with_translation(pos)),
                    PlayerBundle {
                        view_distance: PlayerViewDistance {
                            distance: **distance,
                            ..Default::default()
                        },
                        inventory,
                        game_mode: PlayerGameMode(game_mode),
                        op_level,
                        ..Default::default()
                    },
                ));
                commands.trigger(PlayerJoinEvent { player: entity });
            }
        });
}

/// Per-dim system that materialises an in-dim player entity from an
/// `InboundPlayerSpawn` shuttled across the host→SubApp bus.
///
/// The connection stays host-resident. This system only creates the
/// simulation-side entity and signals the host to bind `in_dim_entity`
/// via `OutboundPlayerAttached`. `PlayerIndex` and `ServerSideConnection`
/// are host-resident and must NOT be accessed here.
fn consume_inbound_player_spawn(
    world_preset: Res<crate::configuration::LoadedWorldPreset>,
    mut reader: MessageReader<InboundPlayerSpawn>,
    mut attached: MessageWriter<OutboundPlayerAttached>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    dims: Query<(Entity, &DimensionId, &DimTypeIndex), With<Dimension>>,
    mut commands: Commands,
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
                    ..Default::default()
                },
                PlayerChunkObserver::default(),
                HostAnchor(spawn.host_anchor),
                GameProfile {
                    id: spawn.snapshot.uuid,
                    username: spawn.snapshot.username.clone(),
                    properties: Vec::new(),
                },
            ))
            .id();

        let host = spawn.host_anchor;
        let wire_id = new_entity.index_u32() as i32;
        let spawn_pos = spawn.snapshot.position;
        let center_x = (spawn_pos.x / 16.0).floor() as i32;
        let center_z = (spawn_pos.z / 16.0).floor() as i32;

        let dimensions: Vec<String> = if world_preset.dimensions.is_empty() {
            vec!["minecraft:overworld".to_string()]
        } else {
            world_preset.dimensions
                .iter()
                .map(|(dim_key, _)| dim_key.as_str().to_owned())
                .collect()
        };

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
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::SetChunkCacheCenter { x: center_x, z: center_z },
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::SetChunkCacheRadius { radius: 12 },
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::LevelChunksLoadStart,
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
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::PlayerPosition {
                teleport_id: 1,
                position: spawn_pos,
            },
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
fn despawn_inbound_player(
    mut reader: MessageReader<InboundPlayerDespawn>,
    players: Query<(Entity, &HostAnchor), With<Player>>,
    mut commands: Commands,
) {
    for msg in reader.read() {
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
        });
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);
    }
}

fn disconnect_player(
    mut players: Query<(&mut ServerSideConnection, &DisconnectReason), With<InGameConnectionState>>,
) {
    players.iter_mut().for_each(|(mut con, reason)| {
        let reason = reason.0.clone();
        con.write_packet(&ClientboundDisconnect { reason })
    })
}

fn despawn_disconnected_clients(
    mut commands: Commands,
    mut disconnected_clients: RemovedComponents<ServerSideConnection>,
) {
    disconnected_clients.read().for_each(|entity| {
        if let Ok(mut entity) = commands.get_entity(entity) {
            entity.insert(Despawned);
        }
    })
}

fn added_inventory(
    mut players: Query<
        (
            &mut ServerSideConnection,
            PlayerInventoryQuery,
            &ContainerSeqno,
        ),
        (With<Player>, Added<ContainerSeqno>),
    >,
    items: Query<&ItemStack >,
) {
    for (mut con, inventory, seqno) in players.iter_mut() {
        let slots = inventory
            .all_slots()
            .iter()
            .map(|slot| {
                slot.and_then(|slot| items.get(slot).ok())
                    .map(|item| Slot::new(item.item_id(), item.count(), ComponentPatch::EMPTY))
                    .unwrap_or(Slot::EMPTY)
            })
            .collect();
        let carried_item = inventory
            .carried_item
            .and_then(|slot| items.get(slot).ok())
            .map(|item| Slot::new(item.item_id(), item.count(), ComponentPatch::EMPTY))
            .unwrap_or(Slot::EMPTY);

        let pkt = ClientboundContainerSetContent {
            container_id: VarInt(0),
            state_seqno: VarInt((**seqno) as i32),
            slot_data: slots,
            carried_item,
        };
        con.write_packet(&pkt);
    }
}

fn resync_player(
    mut players: Query<
        (
            Entity,
            &mut ServerSideConnection,
            &Transform,
            PlayerInventoryQuery,
            &ContainerSeqno,
        ),
        Added<ResyncPlayer>,
    >,
    items: Query<&ItemStack>,
    mut commands: Commands,
) {
    for (entity, mut con, transform, inventory, seqno) in players.iter_mut() {
        // Re-send position
        con.write_packet(&ClientboundPlayerPosition {
            teleport_id: VarInt(0),
            position: transform.translation,
            velocity: DVec3::ZERO,
            look: Look {
                yaw: transform.rotation.y,
                pitch: transform.rotation.x,
            },
            flags: vec![],
        });

        // Re-send inventory
        let slots = inventory
            .all_slots()
            .iter()
            .map(|slot| {
                slot.and_then(|slot| items.get(slot).ok())
                    .map(|item| Slot::new(item.item_id(), item.count(), ComponentPatch::EMPTY))
                    .unwrap_or(Slot::EMPTY)
            })
            .collect();
        let carried_item = inventory
            .carried_item
            .and_then(|slot| items.get(slot).ok())
            .map(|item| Slot::new(item.item_id(), item.count(), ComponentPatch::EMPTY))
            .unwrap_or(Slot::EMPTY);
        con.write_packet(&ClientboundContainerSetContent {
            container_id: VarInt(0),
            state_seqno: VarInt((**seqno) as i32),
            slot_data: slots,
            carried_item,
        });

        commands.entity(entity).remove::<ResyncPlayer>();
    }
}

