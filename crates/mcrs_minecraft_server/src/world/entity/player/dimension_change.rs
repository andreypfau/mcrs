use crate::command::CommandTreePackets;
use crate::command::in_dimension_order;
use crate::world::aoi::components::TrackedBy;
use crate::world::entity::player::ability::{PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::world::entity::player::column_view::ColumnView;
use crate::world::entity::player::movement::TeleportState;
use crate::world::entity::player::{HostAnchor, enter_level_packets, game_mode};
use crate::world::entity::{EntityBundle, EntityUuid};
use crate::world::sub_app_builder::FromDimBacklog;
use bevy_app::{App, SubApp};
use bevy_ecs::change_detection::{DetectChanges, DetectChangesMut, MAX_CHANGE_AGE, Tick};
use bevy_ecs::component::{Component, Mutable};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Mut, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::world::{EntityRef, EntityWorldMut, World};
use bevy_math::DVec3;
use mcrs_minecraft_core::{Id, ResourceKey};
use mcrs_minecraft_dimension::{Dimension as DimensionKey, DimensionType};
use mcrs_minecraft_engine::bridge::OutboundFlush;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, OutboundPlayerReleased};
use mcrs_minecraft_engine::bus::{PacketPayload, to as to_player};
use mcrs_minecraft_engine::channel_types::FromDim;
use mcrs_minecraft_engine::dim::{deliver, pump_dim, receive_from_dim};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_level::session::{DimPlayerIndex, Owner, Place, SessionPlacement};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId, InDimension};
use mcrs_minecraft_level::world::sub_app::DimensionLabel;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundPlayerPosition, ClientboundRespawn,
};
use mcrs_minecraft_protocol::{GameMode, Look, PositionFlags, VarInt};

/// Both bits of the data to keep: the attribute modifiers and the entity data (DIM-CHG-004).
const KEEP_ATTRIBUTES_AND_DATA: u8 = 0x03;

type Carry = fn(&mut EntityWorldMut<'_>, &mut EntityWorldMut<'_>);

/// The components of a player that are its own and go with it to another dimension
/// (DIM-CHG-001), each moved from the entity that leaves to the entity that arrives. A plugin of
/// a dimension world registers the components it owns with
/// [`CarryOnDimensionChange::carry_on_dimension_change`]; a change reads the list of the world
/// that the player leaves. A component not in the list stays behind and is despawned.
#[derive(Resource, Default)]
pub struct CarriedComponents(Vec<Carry>);

pub trait CarryOnDimensionChange {
    /// Moves `T` of a player to its entity in the new world when the player changes dimension.
    fn carry_on_dimension_change<T: Component<Mutability = Mutable>>(&mut self) -> &mut Self;
}

impl CarryOnDimensionChange for App {
    fn carry_on_dimension_change<T: Component<Mutability = Mutable>>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<CarriedComponents>()
            .0
            .push(carry::<T>);
        self
    }
}

/// The moved value is not new to the systems of the new world, and it counts as changed only
/// when a change in the old world was not yet seen there, such as a game mode a command set
/// before the same pass moved the player.
fn carry<T: Component<Mutability = Mutable>>(
    from: &mut EntityWorldMut<'_>,
    to: &mut EntityWorldMut<'_>,
) {
    let Some(unseen_change) = from.get_ref::<T>().map(|value| value.is_changed()) else {
        return;
    };
    let Some(value) = from.take::<T>() else {
        return;
    };
    let long_ago = Tick::new(
        to.world()
            .read_change_tick()
            .get()
            .wrapping_sub(MAX_CHANGE_AGE),
    );
    to.insert(value);
    if let Some(mut moved) = to.get_mut::<T>() {
        moved.set_last_added(long_ago);
        if unseen_change {
            moved.set_changed();
        }
    }
}

/// Where a player arrives: its transform, and what `player_position` tells the client, whose
/// relative axes carry the change and not the value.
#[derive(Clone, Debug, PartialEq)]
pub struct Arrival {
    pub transform: Transform,
    pub position: DVec3,
    pub look: Look,
    pub flags: PositionFlags,
}

impl Arrival {
    /// An arrival that tells the client the absolute position and rotation of `transform`.
    pub fn absolute(transform: Transform) -> Self {
        Self {
            transform,
            position: transform.translation,
            look: Look {
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            },
            flags: PositionFlags::default(),
        }
    }

    pub(crate) fn position_packet(&self, teleport_id: i32) -> ClientboundPlayerPosition {
        ClientboundPlayerPosition {
            teleport_id: VarInt(teleport_id),
            position: self.position,
            velocity: DVec3::ZERO,
            look: self.look,
            flags: self.flags.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DimensionChangeError {
    #[error("the new world holds no dimension")]
    NoDimension,
    #[error("the old world holds no such player")]
    NoPlayer,
    #[error("the player did not leave its dimension")]
    NotLeft,
    #[error("the session of the player is no longer placed in the old dimension")]
    NotPlaced,
    #[error("the player lacks the state its arrival sends")]
    NoArrivalState,
}

/// Moves `player`, an entity of `from`, into the dimension of `to` as the same player, places its
/// session there and sends it the packets of DIM-CHG-003. Gives its entity in `to`. Observers in
/// `from` lose sight of it; its view in `to` starts empty and fills as on a join (DIM-CHG-006,
/// DIM-CHG-007). The host calls this between two ticks, with every world in reach.
pub fn change_dimension(
    host: &mut World,
    from: &mut World,
    to: &mut World,
    player: Entity,
    arrival: Arrival,
) -> Result<Entity, DimensionChangeError> {
    let dimension = to
        .query_filtered::<Entity, With<Dimension>>()
        .single(to)
        .map_err(|_| DimensionChangeError::NoDimension)?;
    let label = to
        .get_resource::<DimensionLabel>()
        .ok_or(DimensionChangeError::NoDimension)?
        .0;
    let leaving = from
        .get_entity(player)
        .map_err(|_| DimensionChangeError::NoPlayer)?;
    let (Some(&anchor), Some(&owner), Some(&uuid), true) = (
        leaving.get::<HostAnchor>(),
        leaving.get::<Owner>(),
        leaving.get::<EntityUuid>(),
        leaving.contains::<Player>(),
    ) else {
        return Err(DimensionChangeError::NoPlayer);
    };
    let state = ArrivalState::read(to, dimension, leaving)?;
    let placed_in_from = from.get_resource::<DimensionLabel>().is_some_and(|from| {
        host.get::<SessionPlacement>(anchor.0)
            .is_some_and(|placement| placement.place().dim() == Some(from.0))
    });
    if !placed_in_from {
        return Err(DimensionChangeError::NotPlaced);
    }
    hand_over_outbound(host, from);
    // The packets already stamped for the player leave before its epoch changes, or the bridge
    // drops them as packets of the dimension it left.
    let _ = host.try_run_schedule(OutboundFlush);
    from.run_system_cached_with(
        crate::world::aoi::drain_player_despawn::leave_dimension,
        player,
    )
    .map_err(|_| DimensionChangeError::NotLeft)?;
    let teleport_id = from
        .get_mut::<TeleportState>(player)
        .ok_or(DimensionChangeError::NoArrivalState)?
        .sent(arrival.transform);
    let arrived = to
        .spawn((
            EntityBundle {
                minecraft_entity: Default::default(),
                dimension: InDimension(dimension),
                transform: arrival.transform,
                uuid,
            },
            anchor,
            owner,
            TrackedBy::default(),
            Player,
            ColumnView::default(),
        ))
        .id();
    from.try_resource_scope(|from, carried: Mut<CarriedComponents>| {
        let (Ok(mut leaving), Ok(mut arriving)) =
            (from.get_entity_mut(player), to.get_entity_mut(arrived))
        else {
            return;
        };
        for carry in &carried.0 {
            carry(&mut leaving, &mut arriving);
        }
    });
    from.despawn(player);
    if let Some(mut index) = from.get_resource_mut::<DimPlayerIndex>() {
        index.0.remove(&owner.0);
    }
    if let Some(mut index) = to.get_resource_mut::<DimPlayerIndex>() {
        index.0.insert(owner.0, arrived);
    }
    if let Some(mut placement) = host.get_mut::<SessionPlacement>(anchor.0) {
        placement.set(Place::InDim(label));
    }
    send_arrival(
        host,
        anchor.0,
        state,
        arrival.position_packet(teleport_id),
        to.get_resource::<CommandTreePackets>(),
    );
    Ok(arrived)
}

/// Stamps everything the old dimension produced and the host did not take yet, in the order it
/// was written. Stamped later, after the epoch changes, the player would get those packets of
/// the dimension it left after its respawn.
fn hand_over_outbound(host: &mut World, from: &mut World) {
    let Some(dim) = from.get_resource::<DimensionLabel>().map(|label| label.0) else {
        return;
    };
    pump_dim(host, dim);
    if let Some(mut backlog) = from.get_resource_mut::<FromDimBacklog>() {
        for message in backlog.0.drain(..) {
            receive_from_dim(host, dim, message);
        }
    }
    if let Some(mut packets) = from.get_resource_mut::<Messages<OutboundPlayerPacket>>() {
        for packet in packets.drain() {
            deliver(host, packet);
        }
    }
    if let Some(mut released) = from.get_resource_mut::<Messages<OutboundPlayerReleased>>() {
        for released in released.drain() {
            receive_from_dim(
                host,
                dim,
                FromDim::Released {
                    session: released.session,
                },
            );
        }
    }
}

/// What the arrival of a player sends, read before the player moves so that a move never
/// leaves the client in the old dimension.
struct ArrivalState {
    mode: GameMode,
    flying: bool,
    op_level: PlayerOpLevel,
    key: ResourceKey<DimensionKey>,
    dimension_type: Id<DimensionType>,
}

impl ArrivalState {
    fn read(
        to: &World,
        dimension: Entity,
        leaving: EntityRef<'_>,
    ) -> Result<Self, DimensionChangeError> {
        let (Some(key), Some(dimension_type)) = (
            to.get::<ResourceKey<DimensionKey>>(dimension),
            to.get::<DimensionTypeId>(dimension),
        ) else {
            return Err(DimensionChangeError::NoDimension);
        };
        let (Some(mode), Some(flying), Some(&op_level), true) = (
            leaving.get::<PlayerGameMode>(),
            leaving.get::<PlayerFlying>(),
            leaving.get::<PlayerOpLevel>(),
            leaving.contains::<TeleportState>(),
        ) else {
            return Err(DimensionChangeError::NoArrivalState);
        };
        Ok(Self {
            mode: mode.0,
            flying: flying.0,
            op_level,
            key: key.clone(),
            dimension_type: dimension_type.0,
        })
    }
}

fn send_arrival(
    host: &mut World,
    anchor: Entity,
    state: ArrivalState,
    position: ClientboundPlayerPosition,
    tree: Option<&CommandTreePackets>,
) {
    deliver(
        host,
        to_player(
            anchor,
            PacketPayload::Respawn(ClientboundRespawn {
                player_spawn_info: PlayerSpawnInfo::new(
                    state.dimension_type,
                    state.key,
                    state.mode,
                ),
                data_to_keep: KEEP_ATTRIBUTES_AND_DATA,
            }),
        )
        .critical(),
    );
    enter_level_packets(
        anchor,
        state.op_level,
        tree,
        position,
        game_mode::abilities(state.mode, state.flying),
        |packet| deliver(host, packet),
    );
}

/// Applies the [`ChangeDimensionRequest`]s that the dimension worlds wrote in their last tick,
/// in the order of the dimensions and then in the order of writing. The server calls it between
/// two ticks, after the pending commands; an embedder that drives the app itself calls it at
/// the same point.
pub fn apply_dimension_change_requests(app: &mut App) {
    let sub_apps = app.sub_apps_mut();
    let requested = sub_apps.sub_apps.values().any(|sub| {
        sub.world()
            .get_resource::<Messages<ChangeDimensionRequest>>()
            .is_some_and(|requests| !requests.is_empty())
    });
    if !requested {
        return;
    }
    let host = sub_apps.main.world_mut();
    let mut worlds = Vec::new();
    in_dimension_order(
        sub_apps.sub_apps.values_mut().map(SubApp::world_mut),
        |world| {
            world
                .query_filtered::<&ResourceKey<DimensionKey>, With<Dimension>>()
                .single(world)
                .ok()
                .cloned()
        },
        |key| key,
        &mut worlds,
    );
    for origin in 0..worlds.len() {
        let Some(requests) = worlds.get_mut(origin).and_then(|(_, world)| {
            let mut requests = world.get_resource_mut::<Messages<ChangeDimensionRequest>>()?;
            Some(requests.drain().collect::<Vec<_>>())
        }) else {
            continue;
        };
        for request in requests {
            let Some(destination) = worlds.iter().position(|(key, _)| *key == request.to) else {
                tracing::error!(to = %request.to, "a dimension change names no running dimension");
                continue;
            };
            let Ok([(_, from), (_, to)]) = worlds.get_disjoint_mut([origin, destination]) else {
                tracing::error!(to = %request.to, "a dimension change names its own dimension");
                continue;
            };
            let arrival = Arrival::absolute(request.transform);
            if let Err(error) = change_dimension(host, from, to, request.player, arrival) {
                tracing::error!(player = ?request.player, "a player did not change dimension: {error}");
            }
        }
    }
}
