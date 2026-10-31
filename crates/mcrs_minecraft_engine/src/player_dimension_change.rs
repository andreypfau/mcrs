use crate::ability::{OpLevelAnnounced, PlayerFlying, PlayerGameMode, PlayerOpLevel};
use crate::bridge::OutboundFlush;
use crate::bus::{self, OutboundPlayerPacket, OutboundPlayerReleased, PacketPayload};
use crate::channel_types::FromDim;
use crate::column_view::ColumnView;
use crate::dim;
use crate::dimension_change::{Arrival, DimensionChangeError};
use crate::dimension_order::{DimensionWorld, in_dimension_order};
use crate::enter_level_packets::enter_level_packets;
use crate::game_mode;
use crate::minecraft_entity::EntityBundle;
use crate::session::HostAnchor;
use crate::sub_app_builder::FromDimBacklog;
use crate::teleport::TeleportState;
use crate::tracked_by::TrackedBy;
use bevy_app::{App, SubApp};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Mut, With};
use bevy_ecs::resource::Resource;
use bevy_ecs::world::{EntityRef, EntityWorldMut, World};
use mcrs_minecraft_core::{Id, ResourceKey};
use mcrs_minecraft_dimension::{Dimension as DimensionKey, DimensionType};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_level::entity::EntityUuid;
use mcrs_minecraft_level::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_level::session::{DimPlayerIndex, Owner, Place, SessionPlacement};
use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeId, InDimension};
use mcrs_minecraft_level::world::sub_app::DimensionLabel;
use mcrs_minecraft_protocol::entity::player::PlayerSpawnInfo;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundPlayerPosition, ClientboundRespawn,
};

/// Both bits of the data to keep: the attribute modifiers and the entity data.
const KEEP_ATTRIBUTES_AND_DATA: u8 = 0x03;

type Carry = fn(&mut EntityWorldMut<'_>, &mut EntityWorldMut<'_>);

/// The components of a player that are its own and go with it to another dimension, each moved from
/// the entity that leaves to the entity that arrives. A plugin of
/// a dimension world registers the components it owns with
/// [`CarryOnDimensionChangeExt::carry_on_dimension_change`]; a change reads the list of the world
/// that the player leaves. A component not in the list stays behind and is despawned.
#[derive(Resource, Default)]
pub(crate) struct CarriedComponents(Vec<Carry>);

/// Registers the components of a player that a plugin of a dimension world owns, so that they go
/// with the player when it changes dimension.
pub trait CarryOnDimensionChangeExt {
    /// Moves `T` of a player to its entity in the new world when the player changes dimension.
    fn carry_on_dimension_change<T: Component>(&mut self) -> &mut Self;
}

impl CarryOnDimensionChangeExt for App {
    fn carry_on_dimension_change<T: Component>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<CarriedComponents>()
            .0
            .push(carry::<T>);
        self
    }
}

fn carry<T: Component>(from: &mut EntityWorldMut<'_>, to: &mut EntityWorldMut<'_>) {
    if let Some(value) = from.take::<T>() {
        to.insert(value);
    }
}

/// Moves `player`, an entity of `from`, into the dimension of `to` as the same player, places its
/// session there and sends it the packets of a respawn. Gives its entity in `to`. Observers in
/// `from` lose sight of it; its view in `to` starts empty and fills as on a join.
/// The host calls this between two ticks, with every world in reach.
pub fn change_dimension(
    host: &mut World,
    from: &mut World,
    to: &mut World,
    player: Entity,
    arrival: Arrival,
) -> Result<Entity, DimensionChangeError> {
    let (dimension, key, dimension_type) = to
        .query_filtered::<(Entity, &ResourceKey<DimensionKey>, &DimensionTypeId), With<Dimension>>()
        .single(to)
        .map(|(dimension, key, dimension_type)| (dimension, key.clone(), dimension_type.0))
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
    let state = ArrivalState::read(key, dimension_type, leaving)?;
    let placed_in_from = from.get_resource::<DimensionLabel>().is_some_and(|from| {
        host.get::<SessionPlacement>(anchor.0)
            .is_some_and(|placement| placement.place().dim() == Some(from.0))
    });
    if !placed_in_from {
        return Err(DimensionChangeError::NotPlaced);
    }
    let teleport_id = leave(host, from, player, &arrival)?;
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
    carry_components(from, to, player, arrived);
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
    let op_level = state.op_level;
    send_arrival(host, anchor.0, state, arrival.position_packet(teleport_id));
    to.write_message(OpLevelAnnounced {
        player: arrived,
        host: anchor.0,
        level: op_level,
    });
    Ok(arrived)
}

/// Takes `player` out of `from`: hands over what `from` produced, takes the player from the
/// view of its observers and claims the teleport id of its arrival.
fn leave(
    host: &mut World,
    from: &mut World,
    player: Entity,
    arrival: &Arrival,
) -> Result<i32, DimensionChangeError> {
    hand_over_outbound(host, from);
    // The packets already stamped for the player leave before its epoch changes, or the bridge
    // drops them as packets of the dimension it left.
    if let Err(error) = host.try_run_schedule(OutboundFlush) {
        tracing::warn!("the packets of a player leaving were not flushed: {error}");
    }
    from.run_system_cached_with(crate::drain_player_despawn::leave_dimension, player)
        .map_err(|_| DimensionChangeError::NotLeft)?;
    Ok(from
        .get_mut::<TeleportState>(player)
        .ok_or(DimensionChangeError::NoArrivalState)?
        .sent(arrival.transform))
}

/// Moves the components of [`CarriedComponents`] of `from` from `leaving` to `arrived` in `to`.
fn carry_components(from: &mut World, to: &mut World, leaving: Entity, arrived: Entity) {
    from.try_resource_scope(|from, carried: Mut<CarriedComponents>| {
        let (Ok(mut leaving), Ok(mut arriving)) =
            (from.get_entity_mut(leaving), to.get_entity_mut(arrived))
        else {
            return;
        };
        for carry in &carried.0 {
            carry(&mut leaving, &mut arriving);
        }
    });
}

/// Stamps everything the old dimension produced and the host did not take yet, in the order it
/// was written. Stamped later, after the epoch changes, the player would get those packets of
/// the dimension it left after its respawn.
fn hand_over_outbound(host: &mut World, from: &mut World) {
    let Some(label) = from.get_resource::<DimensionLabel>().map(|label| label.0) else {
        return;
    };
    dim::pump_dim(host, label);
    if let Some(mut backlog) = from.get_resource_mut::<FromDimBacklog>() {
        for message in backlog.0.drain(..) {
            dim::receive_from_dim(host, label, message);
        }
    }
    if let Some(mut packets) = from.get_resource_mut::<Messages<OutboundPlayerPacket>>() {
        for packet in packets.drain() {
            dim::deliver(host, packet);
        }
    }
    if let Some(mut released) = from.get_resource_mut::<Messages<OutboundPlayerReleased>>() {
        for released in released.drain() {
            dim::receive_from_dim(
                host,
                label,
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
    flying: PlayerFlying,
    op_level: PlayerOpLevel,
    key: ResourceKey<DimensionKey>,
    dimension_type: Id<DimensionType>,
}

impl ArrivalState {
    fn read(
        key: ResourceKey<DimensionKey>,
        dimension_type: Id<DimensionType>,
        leaving: EntityRef<'_>,
    ) -> Result<Self, DimensionChangeError> {
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
            flying: *flying,
            op_level,
            key,
            dimension_type,
        })
    }
}

fn send_arrival(
    host: &mut World,
    anchor: Entity,
    state: ArrivalState,
    position: ClientboundPlayerPosition,
) {
    dim::deliver(
        host,
        bus::to(
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
        position,
        game_mode::abilities(state.mode, state.flying),
        |packet| dim::deliver(host, packet),
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
    let mut requests = Vec::new();
    for origin in 0..worlds.len() {
        requests.clear();
        let Some(DimensionWorld { world, .. }) = worlds.get_mut(origin) else {
            continue;
        };
        let Some(mut written) = world.get_resource_mut::<Messages<ChangeDimensionRequest>>() else {
            continue;
        };
        requests.extend(written.drain());
        for request in requests.drain(..) {
            let Some(destination) = worlds.iter().position(|found| found.read == request.to) else {
                tracing::error!(to = %request.to, "a dimension change names no running dimension");
                continue;
            };
            let Ok([origin_world, destination_world]) =
                worlds.get_disjoint_mut([origin, destination])
            else {
                tracing::error!(to = %request.to, "a dimension change names its own dimension");
                continue;
            };
            let arrival = Arrival::absolute(request.transform);
            let (from, to) = (&mut *origin_world.world, &mut *destination_world.world);
            if let Err(error) = change_dimension(host, from, to, request.player, arrival) {
                tracing::error!(player = ?request.player, "a player did not change dimension: {error}");
            }
        }
    }
}
