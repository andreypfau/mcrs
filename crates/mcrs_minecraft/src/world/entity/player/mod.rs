use crate::client_info::ClientViewDistance;
use crate::configuration::LoadedWorldPreset;
use crate::login::GameProfile;
use crate::world::entity::player::chat::ChatPlugin;
use crate::world::entity::player::column_view::ColumnViewPlugin;
use crate::world::entity::player::digging::DiggingPlugin;
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
use mcrs_network::{ConnectionState, InGameConnectionState, ServerSideConnection};
use mcrs_protocol::entity::player::PlayerSpawnInfo;
use mcrs_protocol::item::ComponentPatch;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundAddEntity, ClientboundContainerSetContent, ClientboundDisconnect,
    ClientboundGameEvent, ClientboundLogin, ClientboundPlayerInfoUpdate,
    ClientboundPlayerPosition,
};
use mcrs_protocol::profile::{PlayerListActions, PlayerListEntry};
use mcrs_protocol::{ByteAngle, GameEventKind, GameMode, Look, Slot, Text, VarInt, WritePacket, ident};
use movement::TeleportState;
use tracing::info;

pub mod ability;
pub mod attribute;
mod chat;
pub(crate) mod column_view;
pub mod digging;
mod inventory;
pub mod movement;
pub mod player_action;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(DiggingPlugin);
        app.add_plugins(PlayerActionPlugin);
        app.add_plugins(MovementPlugin);
        app.add_plugins(ColumnViewPlugin);
        app.add_plugins(PlayerInventoryPlugin);
        app.add_plugins(ChatPlugin);
        app.add_systems(bevy_app::Update, spawn_player);
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
        .for_each(|(entity, distance, con_state, profile, mut con, is_reconfiguration)| {
            if *con_state != ConnectionState::Game {
                return;
            }
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
                        game_mode: GameMode::Survival,
                        ..Default::default()
                    },
                    enforces_secure_chat: false,
                });
                con.write_packet(&ClientboundGameEvent {
                    game_event: GameEventKind::LevelChunksLoadStart,
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
                        game_mode: GameMode::Survival,
                        ..Default::default()
                    },
                    enforces_secure_chat: false,
                });
                con.write_packet(&ClientboundGameEvent {
                    game_event: GameEventKind::LevelChunksLoadStart,
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
                            vert_distance: **distance,
                        },
                        inventory,
                        ..Default::default()
                    },
                ));
                commands.trigger(PlayerJoinEvent { player: entity });
            }
        });
}

#[derive(EntityEvent)]
pub struct PlayerJoinEvent {
    #[event_target]
    pub player: Entity,
}

fn network_add(
    event: On<EntityNetworkAddEvent>,
    added_player: Query<(Entity, &GameProfile, &Transform), With<Player>>,
    mut player: Query<(&mut ServerSideConnection, &Reposition, &GameProfile), With<Player>>,
) {
    let Ok((entity, profile, transform)) = added_player.get(event.entity) else {
        return;
    };
    let Ok((mut connection, reposition, viewer_profile)) = player.get_mut(event.player) else {
        return;
    };

    let pkt = ClientboundAddEntity {
        id: VarInt(entity.index_u32() as i32),
        uuid: profile.id,
        kind: VarInt(MinecraftEntityType::Player as i32),
        pos: reposition.convert_dvec3(transform.translation),
        velocity: VarInt(0),
        yaw: ByteAngle::from_degrees(transform.rotation.y),
        pitch: ByteAngle::from_degrees(transform.rotation.x),
        head_yaw: ByteAngle::from_degrees(transform.rotation.y),
        data: VarInt(0),
    };
    connection.write_packet(&pkt);
    println!(
        "send player {:?} add entity for player viewer: {:?}",
        profile.username, viewer_profile.username
    );
}

fn player_joined(
    event: On<PlayerJoinEvent>,
    mut players: Query<(&mut ServerSideConnection, &GameProfile)>,
    positions: Query<(&Transform), With<Player>>,
) {
    let Ok((con, joined_player)) = players.get(event.player) else {
        return;
    };

    info!(
        "{}[{}] logged in with entity id {} at {}",
        joined_player.username,
        con.remote_addr(),
        event.player,
        positions
            .get(event.player)
            .map(|pos| format!("{}", pos.translation))
            .unwrap_or_default()
    );

    let names = players
        .iter()
        .map(|(_, profile)| profile.username.clone())
        .collect::<Vec<_>>();
    let entries: Vec<PlayerListEntry> = players
        .iter()
        .zip(names.iter())
        .map(|((_, profile), name)| PlayerListEntry {
            username: name.as_str(),
            player_uuid: profile.id,
            properties: profile.properties.iter().cloned().collect(),
            listed: true,
            game_mode: GameMode::Survival,
            ..Default::default()
        })
        .collect();

    let pkt = ClientboundPlayerInfoUpdate {
        actions: PlayerListActions::new()
            .with_add_player(true)
            .with_update_game_mode(true)
            .with_update_listed(true),
        entries: entries.into(),
    };

    players
        .iter_mut()
        .for_each(|(mut connection, _)| connection.write_packet(&pkt));
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
    items: Query<(&ItemStack)>,
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
