use crate::ability::{OpLevelAnnounced, PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::bus::{
    InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerPacket, PacketPayload,
    PlayerInfoEntry, to,
};
use crate::column_view::ColumnView;
use crate::default_game_mode::DefaultGameMode;
use crate::dimension_change::Arrival;
use crate::enter_level_packets::enter_level_packets;
use crate::game_mode::{self, ListedGameMode};
use crate::minecraft_entity::EntityBundle;
use crate::network_id;
use crate::op_level::AnnouncedOpLevel;
use crate::ops::{DefaultOpLevel, OpList};
use crate::persistence;
use crate::player_bundle::PlayerBundle;
use crate::session::{GameProfile, HostAnchor};
use crate::teleport::TeleportState;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, Query, Res, ResMut, With};
use bevy_ecs::world::World;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_engine_world::session::{DimPlayerIndex, Owner};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeId, InDimension};
use mcrs_minecraft_engine_world::world::lifecycle::ticket::SimulationDistance;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundLogin,
};
use tracing::{debug, info};

/// The server clamps what a client asks for to its own limit, as vanilla does; ours is three
/// times vanilla's 32.
const MAX_VIEW_DISTANCE: u8 = 96;

/// What vanilla's player asks for until its client information arrives.
const VIEW_DISTANCE_FALLBACK: u8 = 2;

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
pub(crate) fn consume_inbound_player_spawn(
    mut reader: MessageReader<InboundPlayerSpawn>,
    mut attached: MessageWriter<OutboundPlayerAttached>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
    dims: Query<(Entity, &ResourceKey<DimensionKey>, &DimensionTypeId), With<Dimension>>,
    mut commands: Commands,
    mut dim_index: ResMut<DimPlayerIndex>,
    simulation_distance: Res<SimulationDistance>,
    default_game_mode: Res<DefaultGameMode>,
    ops: Res<OpList>,
    default_op_level: Res<DefaultOpLevel>,
    mut announced_levels: MessageWriter<OpLevelAnnounced>,
) {
    for spawn in reader.read() {
        let Some((dim, dim_key, dimension_type)) = dims.iter().next() else {
            continue;
        };
        let view_distance = PlayerViewDistance {
            distance: spawn
                .snapshot
                .client_info
                .as_ref()
                .map_or(VIEW_DISTANCE_FALLBACK, |info| info.view_distance)
                .clamp(2, MAX_VIEW_DISTANCE),
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
        if let Some(info) = &spawn.snapshot.client_info {
            commands.entity(new_entity).insert(info.clone());
        }
        commands.queue(move |world: &mut World| persistence::load_player(world, new_entity));
        dim_index.0.insert(spawn.session, new_entity);

        let host = spawn.host_anchor;
        let wire_id = network_id::player_network_id(host);
        let spawn_pos = spawn.snapshot.position;

        info!(
            "{} logged in with entity id {wire_id} in {dim_key} at ({:.2}, {:.2}, {:.2})",
            spawn.snapshot.username, spawn_pos.x, spawn_pos.y, spawn_pos.z
        );
        debug!(
            target: "mcrs_minecraft_engine::player",
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
                    radius: VarInt(i32::from(view_distance.distance)),
                }),
            )
            .critical(),
        );
        let arrival = Arrival::absolute(Transform::default().with_translation(spawn_pos));
        enter_level_packets(
            host,
            op_level,
            arrival.position_packet(TeleportState::LOGIN_TELEPORT_ID),
            game_mode::abilities(default_game_mode.0, PlayerFlying(false)),
            |packet| {
                packet_writer.write(packet);
            },
        );

        announced_levels.write(OpLevelAnnounced {
            player: new_entity,
            host,
            level: op_level,
        });
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
                listed_game_mode: ListedGameMode(game_mode),
                announced_op_level: AnnouncedOpLevel(op_level),
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
