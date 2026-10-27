use crate::command::{CommandTree, tree_packet};
use crate::world::aoi::components::TrackedBy;
use crate::world::entity::player::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::world::entity::player::column_view::ColumnView;
use crate::world::entity::player::movement::TeleportState;
use crate::world::entity::player::persistence::{LoadedPlayerDat, UnreadablePlayerDat};
use crate::world::entity::player::{HostAnchor, PlayerBundle, game_mode, op_level_event};
use crate::world::entity::{EntityBundle, EntityUuid};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::In;
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::SystemId;
use bevy_ecs::world::World;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, to};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::chunk_view::PlayerViewDistance;
use mcrs_minecraft_level::session::{DimPlayerIndex, Owner};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId, InDimension};
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundGameEvent, ClientboundPlayerPosition, ClientboundRespawn,
    ClientboundSetChunkCacheCenter,
};
use mcrs_minecraft_protocol::{GameEventKind, VarInt};

/// Both bits of the data to keep: the attribute modifiers and the entity data (DIM-CHG-004).
const KEEP_ATTRIBUTES_AND_DATA: u8 = 0x03;

/// The system that lets a player leave this world, as a disconnect does but without the save.
#[derive(Resource, Clone, Copy)]
pub(crate) struct LeaveDimension(pub(crate) SystemId<In<Entity>>);

/// The state of a player that is its own and goes with it to another dimension (DIM-CHG-001).
/// Its view, its trackers and its dimension belong to the world it is in and start anew there.
#[derive(Bundle)]
struct Carried {
    uuid: EntityUuid,
    profile: GameProfile,
    teleport: TeleportState,
    view_distance: PlayerViewDistance,
    mode: PlayerGameMode,
    flying: PlayerFlying,
    op_level: PlayerOpLevel,
    anchor: HostAnchor,
    owner: Owner,
}

/// Moves `player` out of `from` into `to` at `transform`, the same player with its own state,
/// and gives its entity in `to`. Observers in `from` lose sight of it; the view of the player
/// in `to` starts empty and fills as on a join (DIM-CHG-006, DIM-CHG-007). `None` when `from`
/// does not hold the player or `to` holds no dimension.
pub(crate) fn change_dimension(
    from: &mut World,
    to: &mut World,
    player: Entity,
    transform: Transform,
) -> Option<Entity> {
    let dimension = to
        .query_filtered::<Entity, With<Dimension>>()
        .single(to)
        .ok()?;
    let leave = from.get_resource::<LeaveDimension>().copied()?;
    if let Err(error) = from.run_system_with(leave.0, player) {
        tracing::error!("a player did not leave its dimension: {error}");
        return None;
    }
    let mut leaving = from.get_entity_mut(player).ok()?;
    let carried = leaving.take::<Carried>()?;
    let loaded = leaving.take::<LoadedPlayerDat>();
    let unreadable = leaving.take::<UnreadablePlayerDat>();
    leaving.despawn();
    let session = carried.owner.0;
    if let Some(mut index) = from.get_resource_mut::<DimPlayerIndex>() {
        index.0.remove(&session);
    }
    let Carried {
        uuid,
        profile,
        teleport,
        view_distance,
        mode,
        flying,
        op_level,
        anchor,
        owner,
    } = carried;
    let mut arrived = to.spawn((
        EntityBundle {
            minecraft_entity: Default::default(),
            dimension: InDimension(dimension),
            transform,
            uuid,
        },
        PlayerBundle {
            teleport_state: teleport,
            view_distance,
            game_mode: mode,
            flying,
            op_level,
            tracked_by: TrackedBy::default(),
            marker: Default::default(),
        },
        ColumnView::default(),
        anchor,
        owner,
        profile,
    ));
    if let Some(loaded) = loaded {
        arrived.insert(loaded);
    }
    if let Some(unreadable) = unreadable {
        arrived.insert(unreadable);
    }
    let arrived = arrived.id();
    if let Some(mut index) = to.get_resource_mut::<DimPlayerIndex>() {
        index.0.insert(session, arrived);
    }
    Some(arrived)
}

/// The packets that tell a player it is in the dimension of `world` now, in the order of
/// DIM-CHG-003, with `position` as the move of step 5.
// chisle: steps 2, 8, 9, 10, 13 to 16 and 18 are not sent, because the server holds no
// difficulty, world border, clock packet, spawn position, tick rate packet, inventory sync or post
// effects yet, and the join does not send them either; each step joins this sequence and the
// join together with its state.
pub(crate) fn arrival_packets(
    world: &World,
    player: Entity,
    position: ClientboundPlayerPosition,
    out: &mut Vec<OutboundPlayerPacket>,
) -> Option<()> {
    let entity = world.get_entity(player).ok()?;
    let &HostAnchor(host) = entity.get::<HostAnchor>()?;
    let mode = entity.get::<PlayerGameMode>()?.0;
    let flying = entity.get::<PlayerFlying>()?.0;
    let op_level = *entity.get::<PlayerOpLevel>()?;
    let dimension = entity.get::<InDimension>()?.0;
    let key = world.get::<ResourceKey<mcrs_minecraft_dimension::Dimension>>(dimension)?;
    let dimension_type = world.get::<DimensionTypeId>(dimension)?.0;
    let center = position.position;
    out.push(
        to(
            host,
            PacketPayload::Respawn(ClientboundRespawn {
                player_spawn_info: PlayerSpawnInfo::new(dimension_type, key.clone(), mode),
                data_to_keep: KEEP_ATTRIBUTES_AND_DATA,
            }),
        )
        .critical(),
    );
    out.push(op_level_event(host, op_level));
    if let Some(tree) = world.get_resource::<CommandTree>() {
        out.push(tree_packet(&tree.0, host, op_level));
    }
    out.push(to(host, PacketPayload::PlayerPosition(position)).critical());
    out.push(
        to(
            host,
            PacketPayload::SetChunkCacheCenter(ClientboundSetChunkCacheCenter {
                x: VarInt((center.x / 16.0).floor() as i32),
                z: VarInt((center.z / 16.0).floor() as i32),
            }),
        )
        .critical(),
    );
    out.push(
        to(
            host,
            PacketPayload::PlayerAbilities(game_mode::abilities(mode, flying)),
        )
        .critical(),
    );
    out.push(
        to(
            host,
            PacketPayload::GameEvent(ClientboundGameEvent {
                game_event: GameEventKind::LevelChunksLoadStart,
            }),
        )
        .critical(),
    );
    Some(())
}
