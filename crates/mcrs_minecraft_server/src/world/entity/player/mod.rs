use crate::login::GameProfile;
use crate::ops::{DefaultOpLevel, OpList};
use crate::world::bus::to;
use crate::world::bus::{
    InboundConfirmMove, InboundPlayerDespawn, InboundPlayerSpawn, InboundRollbackMove,
    OutboundPlayerAttached, OutboundPlayerPacket, OutboundPlayerReleased, PacketPayload,
    PlayerInfoEntry,
};
use crate::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use crate::world::entity::player::chat::ChatPlugin;
use crate::world::entity::player::column_view::{ColumnView, ColumnViewPlugin};
use crate::world::entity::player::digging::DiggingPlugin;
use crate::world::entity::player::game_mode::GameModePlugin;
use crate::world::entity::player::inventory::PlayerInventoryPlugin;
use crate::world::entity::player::movement::MovementPlugin;
use crate::world::entity::player::placing::PlacingPlugin;
use crate::world::entity::player::player_action::PlayerActionPlugin;
use crate::world::entity::{EntityBundle, EntityUuid};
use crate::world::inventory::PlayerInventoryBundle;
use crate::world::item::StackSet;
use bevy_app::{FixedUpdate, Plugin, Update};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Changed, Commands, Query, Res, ResMut, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_inventory::{Op, Slot};
use mcrs_minecraft_item::{SlotTable, slots};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_level::aoi::every_n_ticks;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_level::entity::{Despawned, EntityNetworkAddEvent, InTransit};
use mcrs_minecraft_level::session::{DimPlayerIndex, Owner};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId, InDimension};
use mcrs_minecraft_level::world::lifecycle::ticket::SimulationDistance;
use mcrs_minecraft_protocol::ByteAngle;
use mcrs_minecraft_protocol::GameEventKind;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::Look;
use mcrs_minecraft_protocol::LpVec3;
use mcrs_minecraft_protocol::RegistryId;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundChunkCacheRadius;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityEvent;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundLogin;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSetChunkCacheCenter;
use movement::TeleportState;
use tracing::{debug, info};

pub mod ability;
mod chat;
pub mod column_view;
pub mod digging;
mod game_mode;
mod inventory;
pub mod movement;
pub mod persistence;
mod placing;
pub mod player_action;

/// Game mode given to joining players, read once from `MCRS_DEFAULT_GAMEMODE`
/// (`survival`, `creative`, `adventure`, or `spectator`). Falls back to creative
/// when unset or unrecognized.
#[derive(Resource, Clone, Copy, Debug)]
pub struct DefaultGameMode(pub GameMode);

fn game_mode_from_env() -> GameMode {
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
/// sessions or `ServerSideConnection`.
#[derive(bevy_ecs::component::Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAnchor(pub Entity);

/// Plugin for per-dimension worlds. Registers only the bus-driven and
/// simulation systems that are safe to run inside a DimWorld — no
/// `ServerSideConnection` or other host-only resource is accessed.
pub struct DimPlayerPlugin;

impl Plugin for DimPlayerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.insert_resource(DefaultGameMode(game_mode_from_env()));
        app.add_plugins(DiggingPlugin);
        app.add_plugins(PlayerActionPlugin);
        app.add_plugins(MovementPlugin);
        app.add_plugins(ColumnViewPlugin);
        app.add_plugins(PlayerInventoryPlugin);
        app.add_plugins(PlacingPlugin);
        app.add_plugins(ChatPlugin);
        app.add_plugins(GameModePlugin);
        app.add_systems(
            Update,
            (
                (consume_inbound_player_spawn, send_op_level).chain(),
                // A player spawned and despawned in one drain must be saved and removed, or it
                // lingers and its autosaves overwrite the next session's file.
                despawn_inbound_player.after(consume_inbound_player_spawn),
            ),
        );
        app.add_systems(FixedUpdate, (despawn_on_confirm, unhide_on_rollback));
        app.add_systems(
            FixedUpdate,
            persistence::autosave_players
                .after(StackSet::Sync)
                .run_if(every_n_ticks(persistence::AUTOSAVE_INTERVAL)),
        );
        app.add_observer(network_add);
    }
}

/// The server clamps what a client asks for to its own limit, as vanilla does; ours is three
/// times vanilla's 32.
const MAX_VIEW_DISTANCE: u8 = 96;

#[derive(Bundle, Default)]
pub struct PlayerBundle {
    pub teleport_state: TeleportState,
    pub view_distance: PlayerViewDistance,
    pub abilities: ability::PlayerAbilitiesBundle,
    pub inventory: PlayerInventoryBundle,
    pub game_mode: PlayerGameMode,
    pub op_level: PlayerOpLevel,
    pub tracked_by: crate::world::aoi::TrackedBy,
    pub marker: Player,
}

/// Per-dim system that materialises an in-dim player entity from an
/// `InboundPlayerSpawn` shuttled across the host→SubApp bus.
///
/// The connection stays host-resident. This system only creates the
/// simulation-side entity and signals the host to attach the session via
/// `OutboundPlayerAttached`. Sessions and `ServerSideConnection` are
/// host-resident and must NOT be accessed here.
fn consume_inbound_player_spawn(
    mut reader: MessageReader<InboundPlayerSpawn>,
    mut attached: MessageWriter<OutboundPlayerAttached>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    dims: Query<(Entity, &ResourceKey<mcrs_minecraft_dimension::Dimension>, &DimensionTypeId), With<Dimension>>,
    mut commands: Commands,
    mut dim_index: ResMut<DimPlayerIndex>,
    simulation_distance: Res<SimulationDistance>,
    default_game_mode: Res<DefaultGameMode>,
    ops: Res<OpList>,
    default_op_level: Res<DefaultOpLevel>,
) {
    for spawn in reader.read() {
        let Some((dim, dim_key, dimension_type)) = dims.iter().next() else {
            continue;
        };
        let view_distance = PlayerViewDistance {
            distance: spawn.snapshot.view_distance.clamp(2, MAX_VIEW_DISTANCE),
            ..Default::default()
        };
        let new_entity = commands
            .spawn((
                EntityBundle {
                    minecraft_entity: Default::default(),
                    dimension: InDimension(dim),
                    transform: Transform::default().with_translation(spawn.snapshot.position),
                    uuid: EntityUuid(spawn.snapshot.uuid),
                },
                PlayerBundle {
                    game_mode: PlayerGameMode(default_game_mode.0),
                    op_level: ops.level_of(&spawn.snapshot.uuid, *default_op_level),
                    teleport_state: TeleportState::after_login(),
                    view_distance,
                    ..Default::default()
                },
                ColumnView::default(),
                HostAnchor(spawn.host_anchor),
                Owner(spawn.session),
                GameProfile {
                    id: spawn.snapshot.uuid,
                    username: spawn.snapshot.username.clone(),
                    properties: Vec::new(),
                },
            ))
            .id();
        commands.queue(move |world: &mut World| persistence::load_player(world, new_entity));
        dim_index.0.insert(spawn.session, new_entity);

        let host = spawn.host_anchor;
        let wire_id = new_entity.index_u32() as i32;
        let spawn_pos = spawn.snapshot.position;
        let center_x = (spawn_pos.x / 16.0).floor() as i32;
        let center_z = (spawn_pos.z / 16.0).floor() as i32;

        let dimensions = spawn.dimensions.to_vec();

        info!(
            "{} logged in with entity id {wire_id} in {dim_key} at ({:.2}, {:.2}, {:.2})",
            spawn.snapshot.username, spawn_pos.x, spawn_pos.y, spawn_pos.z
        );
        debug!(
            target: "mcrs_minecraft_server::player",
            player = wire_id,
            host_anchor = ?host,
            "emit_play_login: emitting play ClientboundLogin for newly-materialized in-dim entity"
        );

        packet_writer.write(
            to(
                host,
                PacketPayload::PlayerLogin(ClientboundLogin {
                    player_id: wire_id,
                    hardcore: false,
                    dimensions,
                    max_players: VarInt(100),
                    chunk_radius: VarInt(view_distance.distance as i32),
                    simulation_distance: VarInt(simulation_distance.0 as i32),
                    reduced_debug_info: false,
                    show_death_screen: false,
                    do_limited_crafting: false,
                    player_spawn_info: PlayerSpawnInfo {
                        dimension_type_id: RegistryId(dimension_type.0.number()),
                        dimension: dim_key.clone(),
                        game_mode: default_game_mode.0,
                        ..Default::default()
                    },
                    online_mode: false,
                    enforces_secure_chat: false,
                }),
            )
            .critical(),
        );

        // The client derives the local player's game mode (and therefore
        // spectator noclip) from its own player-list entry, not the login
        // packet. Without this the client treats itself as non-spectator and
        // keeps block collisions even though login set the spectator mode.
        packet_writer.write(
            to(
                host,
                PacketPayload::PlayerInfoUpdate {
                    entries: vec![PlayerInfoEntry {
                        player_uuid: spawn.snapshot.uuid,
                        username: spawn.snapshot.username.clone(),
                        game_mode: default_game_mode.0,
                        listed: true,
                    }],
                },
            )
            .critical(),
        );

        packet_writer.write(
            to(
                host,
                PacketPayload::SetChunkCacheCenter(ClientboundSetChunkCacheCenter {
                    x: VarInt(center_x),
                    z: VarInt(center_z),
                }),
            )
            .critical(),
        );

        packet_writer.write(
            to(
                host,
                PacketPayload::SetChunkCacheRadius(ClientboundChunkCacheRadius {
                    radius: VarInt(view_distance.distance as i32),
                }),
            )
            .critical(),
        );

        packet_writer.write(
            to(
                host,
                PacketPayload::GameEvent(ClientboundGameEvent {
                    game_event: GameEventKind::LevelChunksLoadStart,
                }),
            )
            .critical(),
        );

        packet_writer.write(
            to(
                host,
                PacketPayload::PlayerPosition(ClientboundPlayerPosition {
                    teleport_id: VarInt(TeleportState::LOGIN_TELEPORT_ID),
                    position: spawn_pos,
                    velocity: DVec3::ZERO,
                    look: Look::default(),
                    flags: Vec::new(),
                }),
            )
            .critical(),
        );

        attached.write(OutboundPlayerAttached {
            host_anchor: spawn.host_anchor,
        });
    }
}

fn send_op_level(
    players: Query<(Entity, &PlayerOpLevel, &HostAnchor), Changed<PlayerOpLevel>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (entity, &op_level, &HostAnchor(host)) in &players {
        packet_writer.write(
            to(
                host,
                PacketPayload::OpLevelEntityEvent(ClientboundEntityEvent {
                    entity_id: entity.index_u32() as i32,
                    entity_status: op_level.entity_status(),
                }),
            )
            .critical(),
        );
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
                commands.queue(move |world: &mut World| {
                    let unsaved: Vec<Op> = world
                        .get::<SlotTable>(entity)
                        .map(|table| {
                            std::iter::once(slots::CARRIED)
                                .chain(slots::CRAFT)
                                .filter(|index| table.get(*index).is_some())
                                .map(|index| Op::Drop {
                                    from: Slot::new(entity, index),
                                    count: u8::MAX,
                                    thrower: entity,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    crate::world::item::click::commit(world, unsaved);
                    persistence::write_player(world, entity);
                    world.despawn(entity);
                });
            }
        }
        // Queued after the save so the host never learns of the release before the file is written.
        let session = msg.session;
        commands.queue(move |world: &mut World| {
            world.write_message(OutboundPlayerReleased { session });
        });
    }
}

fn network_add(
    event: On<EntityNetworkAddEvent>,
    added_player: Query<(Entity, &GameProfile, &Transform), With<Player>>,
    viewer: Query<&HostAnchor, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((entity, profile, transform)) = added_player.get(event.entity) else {
        return;
    };
    let Ok(&HostAnchor(host_anchor)) = viewer.get(event.player) else {
        return;
    };

    packet_writer.write(to(
        host_anchor,
        PacketPayload::PlayerEnteredView(ClientboundAddEntity {
            id: VarInt(entity.index_u32() as i32),
            uuid: profile.id,
            kind: RegistryId::from(keys::entity_type::PLAYER.id()),
            pos: transform.translation,
            movement: LpVec3(DVec3::ZERO),
            yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
            pitch: ByteAngle::from_degrees(transform.rotation.pitch()),
            head_yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
            data: VarInt(0),
        }),
    ));
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
                commands
                    .entity(entity)
                    .remove::<InTransit>()
                    .insert(Despawned);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::schedule::ScheduleBuildError;

    #[test]
    fn a_despawn_is_handled_after_a_spawn_drained_in_the_same_tick() {
        let mut app = App::new();
        app.add_plugins(DimPlayerPlugin);
        // This closes a cycle exactly when the spawn is ordered before the despawn.
        app.add_systems(
            Update,
            (|| {})
                .after(despawn_inbound_player)
                .before(consume_inbound_player_spawn),
        );
        let built = app.world_mut().schedule_scope(Update, |world, schedule| {
            schedule.initialize(world).map(|_| ())
        });
        assert!(
            matches!(
                built,
                Err(ScheduleBuildError::CrossDependency(_) | ScheduleBuildError::DependencySort(_))
            ),
            "{built:?}"
        );
    }
}
