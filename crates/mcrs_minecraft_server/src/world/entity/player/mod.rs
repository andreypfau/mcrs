use crate::ops::{DefaultOpLevel, OpList};
use crate::world::entity::player::chat::ChatPlugin;
use crate::world::entity::player::column_view::{ColumnView, ColumnViewPlugin};
use crate::world::entity::player::dimension_change::CarryOnDimensionChangeExt;
use crate::world::entity::player::game_mode::GameModePlugin;
use crate::world::entity::player::movement::MovementPlugin;
use crate::world::entity::{EntityBundle, EntityUuid};
use bevy_app::{FixedUpdate, Plugin, Update};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Changed, Commands, DetectChanges, Query, Ref, Res, ResMut, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_core::{ColumnPos, ResourceKey};
use mcrs_minecraft_engine::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerPacket,
    OutboundPlayerReleased, PacketPayload, PlayerInfoEntry,
};
use mcrs_minecraft_engine::dimension_change::DimensionChanger;
use mcrs_minecraft_engine::session::{GameProfile, HostAnchor};
use mcrs_minecraft_engine::teleport::TeleportState;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_level::aoi::every_n_ticks;
use mcrs_minecraft_level::entity::EntityNetworkAddEvent;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_level::session::{DimPlayerIndex, Owner};
use mcrs_minecraft_level::tick::TickSet;
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId, InDimension};
use mcrs_minecraft_level::world::lifecycle::ticket::SimulationDistance;
use mcrs_minecraft_protocol::ByteAngle;
use mcrs_minecraft_protocol::GameEventKind;
use mcrs_minecraft_protocol::Look;
use mcrs_minecraft_protocol::LpVec3;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundAddEntity;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundChunkCacheRadius;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityEvent;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundLogin;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerAbilities;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundSetChunkCacheCenter;
use tracing::{debug, info};

mod chat;
pub mod column_view;
pub mod dimension_change;
pub(crate) mod game_mode;
pub mod movement;
pub mod persistence;

/// Game mode given to players joining for the first time.
#[derive(Resource, Clone, Copy, Debug)]
pub struct DefaultGameMode(pub GameMode);

impl Default for DefaultGameMode {
    fn default() -> Self {
        Self(GameMode::Creative)
    }
}

/// Plugin for per-dimension worlds. Registers only the bus-driven and
/// simulation systems that are safe to run inside a DimWorld — no
/// `ServerSideConnection` or other host-only resource is accessed.
pub struct DimPlayerPlugin;

impl Plugin for DimPlayerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<DefaultGameMode>();
        app.insert_resource(DimensionChanger(dimension_change::change_dimension));
        app.add_plugins(MovementPlugin);
        app.add_plugins(ColumnViewPlugin);
        app.add_plugins(ChatPlugin);
        app.add_plugins(GameModePlugin);
        app.carry_on_dimension_change::<GameProfile>()
            .carry_on_dimension_change::<PlayerViewDistance>()
            .carry_on_dimension_change::<PlayerOpLevel>()
            .carry_on_dimension_change::<persistence::LoadedPlayerDat>()
            .carry_on_dimension_change::<persistence::UnreadablePlayerDat>();
        app.add_systems(
            Update,
            (
                (
                    consume_inbound_player_spawn,
                    send_op_level,
                    game_mode::send_game_mode,
                )
                    .chain(),
                // A player spawned and despawned in one drain must be saved and removed, or it
                // lingers and its autosaves overwrite the next session's file.
                despawn_inbound_player.after(consume_inbound_player_spawn),
            ),
        );
        app.add_systems(
            FixedUpdate,
            persistence::autosave_players
                .in_set(TickSet::Output)
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
    pub game_mode: PlayerGameMode,
    pub flying: PlayerFlying,
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
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
fn consume_inbound_player_spawn(
    mut reader: MessageReader<InboundPlayerSpawn>,
    mut attached: MessageWriter<OutboundPlayerAttached>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    dims: Query<
        (
            Entity,
            &ResourceKey<mcrs_minecraft_dimension::Dimension>,
            &DimensionTypeId,
        ),
        With<Dimension>,
    >,
    mut commands: Commands,
    mut dim_index: ResMut<DimPlayerIndex>,
    simulation_distance: Res<SimulationDistance>,
    default_game_mode: Res<DefaultGameMode>,
    ops: Res<OpList>,
    default_op_level: Res<DefaultOpLevel>,
    tree: Option<Res<mcrs_minecraft_command_engine::CommandTreePackets>>,
) {
    for spawn in reader.read() {
        let Some((dim, dim_key, dimension_type)) = dims.iter().next() else {
            continue;
        };
        let view_distance = PlayerViewDistance {
            distance: spawn.snapshot.view_distance.clamp(2, MAX_VIEW_DISTANCE),
            ..Default::default()
        };
        let op_level = ops.level_of(&spawn.snapshot.uuid, *default_op_level);
        let new_entity = spawn_player_entity(
            &mut commands,
            spawn,
            dim,
            view_distance,
            op_level,
            default_game_mode.0,
        );
        commands.queue(move |world: &mut World| persistence::load_player(world, new_entity));
        dim_index.0.insert(spawn.session, new_entity);

        let host = spawn.host_anchor;
        let wire_id = mcrs_minecraft_engine::network_id::player_network_id(host);
        let spawn_pos = spawn.snapshot.position;

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
                login_packet(
                    spawn,
                    wire_id,
                    PlayerSpawnInfo::new(dimension_type.0, dim_key.clone(), default_game_mode.0),
                    view_distance.distance,
                    simulation_distance.0,
                ),
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
                PacketPayload::SetChunkCacheRadius(ClientboundChunkCacheRadius {
                    radius: VarInt(view_distance.distance as i32),
                }),
            )
            .critical(),
        );
        enter_level_packets(
            host,
            op_level,
            tree.as_deref(),
            ClientboundPlayerPosition {
                teleport_id: VarInt(TeleportState::LOGIN_TELEPORT_ID),
                position: spawn_pos,
                velocity: DVec3::ZERO,
                look: Look::default(),
                flags: mcrs_minecraft_protocol::PositionFlags::default(),
            },
            game_mode::abilities(default_game_mode.0, false),
            |packet| {
                packet_writer.write(packet);
            },
        );

        attached.write(OutboundPlayerAttached {
            host_anchor: spawn.host_anchor,
        });
    }
}

fn login_packet(
    spawn: &InboundPlayerSpawn,
    wire_id: i32,
    player_spawn_info: PlayerSpawnInfo,
    view_distance: u8,
    simulation_distance: u8,
) -> PacketPayload {
    PacketPayload::PlayerLogin(ClientboundLogin {
        player_id: wire_id,
        hardcore: false,
        dimensions: spawn.dimensions.to_vec(),
        max_players: VarInt(100),
        chunk_radius: VarInt(i32::from(view_distance)),
        simulation_distance: VarInt(i32::from(simulation_distance)),
        reduced_debug_info: false,
        show_death_screen: false,
        do_limited_crafting: false,
        player_spawn_info,
        online_mode: false,
        enforces_secure_chat: false,
    })
}

fn spawn_player_entity(
    commands: &mut Commands,
    spawn: &InboundPlayerSpawn,
    dim: Entity,
    view_distance: PlayerViewDistance,
    op_level: PlayerOpLevel,
    game_mode: GameMode,
) -> Entity {
    commands
        .spawn((
            EntityBundle {
                minecraft_entity: Default::default(),
                dimension: InDimension(dim),
                transform: Transform::default().with_translation(spawn.snapshot.position),
                uuid: EntityUuid(spawn.snapshot.uuid),
            },
            PlayerBundle {
                game_mode: PlayerGameMode(game_mode),
                op_level,
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
        .id()
}

/// A player that spawns or changes dimension learns its level in the sequence that brings it
/// in; this tells it every later change, with the tree of its new level.
pub(crate) fn send_op_level(
    players: Query<(Ref<PlayerOpLevel>, &HostAnchor), Changed<PlayerOpLevel>>,
    tree: Option<Res<mcrs_minecraft_command_engine::CommandTreePackets>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (op_level, &HostAnchor(host)) in &players {
        if !op_level.is_added() {
            level_packets(host, *op_level, tree.as_deref(), |packet| {
                packet_writer.write(packet);
            });
        }
    }
}

/// The level of a player, then the command tree it can see at that level (CMDP-TREE-002).
pub(crate) fn level_packets(
    host: Entity,
    op_level: PlayerOpLevel,
    tree: Option<&mcrs_minecraft_command_engine::CommandTreePackets>,
    mut push: impl FnMut(OutboundPlayerPacket),
) {
    push(
        to(
            host,
            PacketPayload::OpLevelEntityEvent(ClientboundEntityEvent {
                entity_id: mcrs_minecraft_engine::network_id::player_network_id(host),
                entity_status: op_level.entity_status(),
            }),
        )
        .critical(),
    );
    if let Some(packet) = tree.and_then(|tree| tree.to_player(host, op_level)) {
        push(packet);
    }
}

/// The packets that put a player into the level of its world, after the `login` of a join or
/// the `respawn` of a dimension change: steps 3 to 7 and 12 of DIM-CHG-003, with `position` as
/// the move of step 5.
// chisle: steps 2, 8, 9, 10, 13 to 16 and 18 are not sent, because the server holds no
// difficulty, world border, clock packet, spawn position, tick rate packet, inventory sync or post
// effects yet; each step joins this sequence with its state.
pub(crate) fn enter_level_packets(
    host: Entity,
    op_level: PlayerOpLevel,
    tree: Option<&mcrs_minecraft_command_engine::CommandTreePackets>,
    position: ClientboundPlayerPosition,
    abilities: ClientboundPlayerAbilities,
    mut push: impl FnMut(OutboundPlayerPacket),
) {
    let column = ColumnPos::from(position.position);
    level_packets(host, op_level, tree, &mut push);
    push(to(host, PacketPayload::PlayerPosition(position)).critical());
    push(
        to(
            host,
            PacketPayload::SetChunkCacheCenter(ClientboundSetChunkCacheCenter {
                x: VarInt(column.x),
                z: VarInt(column.z),
            }),
        )
        .critical(),
    );
    push(to(host, PacketPayload::PlayerAbilities(abilities)).critical());
    push(
        to(
            host,
            PacketPayload::GameEvent(ClientboundGameEvent {
                game_event: GameEventKind::LevelChunksLoadStart,
            }),
        )
        .critical(),
    );
}

/// Per-dim consumer that despawns the in-dim player entity when an
/// `InboundPlayerDespawn` arrives for its host anchor on a disconnect, so the
/// dimension stops streaming chunks toward that connection.
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
    added_player: Query<(&GameProfile, &Transform, &HostAnchor), With<Player>>,
    viewer: Query<&HostAnchor, With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((profile, transform, &HostAnchor(anchor))) = added_player.get(event.entity) else {
        return;
    };
    let Ok(&HostAnchor(host_anchor)) = viewer.get(event.player) else {
        return;
    };

    packet_writer.write(to(
        host_anchor,
        PacketPayload::PlayerEnteredView(ClientboundAddEntity {
            id: VarInt(mcrs_minecraft_engine::network_id::player_network_id(anchor)),
            uuid: profile.id,
            kind: mcrs_minecraft_entity::keys::EntityType::Player,
            pos: transform.translation,
            movement: LpVec3(DVec3::ZERO),
            yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
            pitch: ByteAngle::from_degrees(transform.rotation.pitch()),
            head_yaw: ByteAngle::from_degrees(transform.rotation.yaw()),
            data: VarInt(0),
        }),
    ));
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
