use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::{Dimension as DimensionKey, DimensionType};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_engine_world::session::{
    Owner, Place, PlayerSession, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeId, InDimension};
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRespawn;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
use mcrs_minecraft_resource::{Id, ResourceKeyBuf};

use crate::ability::OpLevelAnnounced;
use crate::bridge::OutboundFlush;
use crate::bus::{self, PacketPayload};
use crate::channel_types::ToDim;
use crate::column_view::ColumnView;
use crate::departure::{CarriedPlayer, Departure, PlayerTransfer};
use crate::dim;
use crate::dim::DimSubAppHandle;
use crate::dim_inputs::DimInputs;
use crate::dimension_change::{Arrival, DimensionChangeError};
use crate::enter_level_packets::enter_level_packets;
use crate::game_mode;
use crate::minecraft_entity::EntityBundle;

/// Both bits of the data to keep: the attribute modifiers and the entity data.
const KEEP_ATTRIBUTES_AND_DATA: u8 = 0x03;

/// Lets go of each player that a [`ChangeDimensionRequest`] of this tick names, in the order of
/// writing, into `departures`. A request that this world cannot meet leaves the player where it
/// is.
pub(crate) fn depart_players(world: &mut World, departures: &mut Vec<Departure>) {
    let Some(mut written) = world.get_resource_mut::<Messages<ChangeDimensionRequest>>() else {
        return;
    };
    if written.is_empty() {
        return;
    }
    let requests: Vec<ChangeDimensionRequest> = written.drain().collect();
    let own = world
        .query_filtered::<&ResourceKeyBuf<DimensionKey>, With<Dimension>>()
        .single(world)
        .ok()
        .cloned();
    for request in requests {
        if own.as_ref() == Some(&request.to) {
            tracing::error!(to = %request.to, "a dimension change names its own dimension");
            continue;
        }
        let arrival = Arrival::absolute(request.transform);
        match depart(world, request.player, request.to, arrival) {
            Ok(departure) => departures.push(departure),
            Err(error) => {
                tracing::error!(player = ?request.player, "a player did not change dimension: {error}");
            }
        }
    }
}

/// Takes the carried state of `player` and despawns it. Its viewers here lose it.
fn depart(
    world: &mut World,
    player: Entity,
    to: ResourceKeyBuf<DimensionKey>,
    arrival: Arrival,
) -> Result<Departure, DimensionChangeError> {
    let mut leaving = world
        .get_entity_mut(player)
        .map_err(|_| DimensionChangeError::NoPlayer)?;
    let Some(&Owner(session)) = leaving.get::<Owner>() else {
        return Err(DimensionChangeError::NoPlayer);
    };
    let carried = CarriedPlayer::take(&mut leaving)?;
    leaving.despawn();
    Ok(Departure {
        session,
        to,
        arrival,
        carried,
    })
}

/// Spawns the players that the host handed this world in the input of this tick, before their
/// serverbound packets of the same tick reach them.
pub(crate) fn receive_transfers(world: &mut World) {
    let Some(mut messages) = world.get_resource_mut::<Messages<PlayerTransfer>>() else {
        return;
    };
    let transfers: Vec<PlayerTransfer> = messages.drain().collect();
    for transfer in transfers {
        match transfer {
            PlayerTransfer::Arrive(departure) => {
                if let Err(error) = arrive(world, departure) {
                    tracing::error!("a player did not arrive: {error}");
                }
            }
            PlayerTransfer::Return(departure) => {
                let transform = departure.carried.transform;
                if let Err(error) = place(world, departure.session, transform, departure.carried) {
                    tracing::error!("a player did not return: {error}");
                }
            }
        }
    }
}

/// Spawns the player of `departure` at its arrival. Its view starts empty and fills as on a
/// join.
fn arrive(world: &mut World, departure: Departure) -> Result<Entity, DimensionChangeError> {
    let Departure {
        session,
        arrival,
        carried,
        ..
    } = departure;
    let level = carried.op_level;
    let arrived = place(world, session, arrival.transform, carried)?;
    world.write_message(OpLevelAnnounced {
        player: arrived,
        session,
        level,
    });
    Ok(arrived)
}

fn place(
    world: &mut World,
    session: PlayerSession,
    transform: Transform,
    carried: CarriedPlayer,
) -> Result<Entity, DimensionChangeError> {
    let dimension = world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .map_err(|_| DimensionChangeError::NoDimension)?;
    let mut player = world.spawn((
        EntityBundle {
            minecraft_entity: Default::default(),
            dimension: InDimension(dimension),
            transform,
            uuid: carried.uuid,
        },
        Owner(session),
        Player,
        ColumnView::default(),
    ));
    carried.insert_into(&mut player);
    Ok(player.id())
}

/// The dimension a player arrives in, as the host knows it.
struct Destination {
    label: Entity,
    key: ResourceKeyBuf<DimensionKey>,
    dimension_type: Id<DimensionType>,
}

fn destination(host: &mut World, to: &ResourceKeyBuf<DimensionKey>) -> Option<Destination> {
    host.query_filtered::<(Entity, &ResourceKeyBuf<DimensionKey>, &DimensionTypeId), With<DimSubAppHandle>>()
        .iter(host)
        .find(|(_, key, _)| *key == to)
        .map(|(label, key, dimension_type)| Destination {
            label,
            key: key.clone(),
            dimension_type: dimension_type.0,
        })
}

/// The host anchor of `session` while the session is placed in the dimension `source`.
fn placed_anchor(host: &World, session: PlayerSession, source: Entity) -> Option<Entity> {
    host.get_resource::<SessionAnchors>()
        .and_then(|anchors| anchors.get(session))
        .filter(|&anchor| {
            host.get::<SessionPlacement>(anchor)
                .is_some_and(|placement| placement.place().dim() == Some(source))
        })
}

/// Sends `departure`, which the dimension `source` handed over, on to the dimension it names
/// for its next tick, and sends the client the packets of a respawn. A departure that names no
/// other running dimension, or whose session is no longer placed in `source`, goes back to
/// `source`.
pub(crate) fn route_departure(host: &mut World, source: Entity, mut departure: Departure) {
    let anchor = placed_anchor(host, departure.session, source);
    let destination = destination(host, &departure.to).filter(|found| found.label != source);
    let (Some(anchor), Some(destination)) = (anchor, destination) else {
        tracing::error!(
            to = %departure.to,
            session = ?departure.session,
            "a player did not change dimension and returns where it was"
        );
        send(host, source, PlayerTransfer::Return(departure));
        return;
    };
    let label = destination.label;
    announce(host, anchor, destination, &mut departure);
    send(host, label, PlayerTransfer::Arrive(departure));
}

fn send(host: &mut World, dim: Entity, transfer: PlayerTransfer) {
    let sent = host
        .get_resource_mut::<DimInputs>()
        .is_some_and(|mut inputs| inputs.send_control(dim, ToDim::Transfer(Box::new(transfer))));
    if !sent {
        tracing::error!(?dim, "a dimension that a player changes to takes no input");
    }
}

/// Places the session of `departure` in `destination` and sends it the packets of a respawn
/// there, after the packets of the dimension it leaves.
fn announce(host: &mut World, anchor: Entity, destination: Destination, departure: &mut Departure) {
    // The packets stamped for the dimension the player leaves go out before its epoch changes,
    // or the bridge drops them as stale.
    if let Err(error) = host.try_run_schedule(OutboundFlush) {
        tracing::warn!("the packets of a player leaving were not flushed: {error}");
    }
    if let Some(mut placement) = host.get_mut::<SessionPlacement>(anchor) {
        placement.set(Place::InDim(destination.label));
    }
    let session = departure.session;
    let carried = &mut departure.carried;
    let teleport_id = carried.teleport.sent(departure.arrival.transform);
    dim::deliver(
        host,
        bus::to(
            session,
            PacketPayload::Respawn(ClientboundRespawn {
                player_spawn_info: PlayerSpawnInfo::new(
                    destination.dimension_type,
                    destination.key,
                    carried.game_mode.0,
                ),
                data_to_keep: KEEP_ATTRIBUTES_AND_DATA,
            }),
        )
        .critical(),
    );
    enter_level_packets(
        session,
        carried.op_level,
        departure.arrival.position_packet(teleport_id),
        game_mode::abilities(carried.game_mode.0, carried.flying),
        |packet| dim::deliver(host, packet),
    );
}

/// Moves `player`, an entity of `from`, into the dimension of `to` at once, through the same
/// departure that a [`ChangeDimensionRequest`] makes. Gives its entity in `to`. A caller that
/// holds every world between two ticks uses this, so that the player is in `to` for the rest of
/// what the caller does.
pub fn change_dimension(
    host: &mut World,
    from: &mut World,
    to: &mut World,
    player: Entity,
    arrival: Arrival,
) -> Result<Entity, DimensionChangeError> {
    let source = from
        .get_resource::<DimensionLabel>()
        .ok_or(DimensionChangeError::NoPlayer)?
        .0;
    let key = to
        .query_filtered::<&ResourceKeyBuf<DimensionKey>, With<Dimension>>()
        .single(to)
        .map_err(|_| DimensionChangeError::NoDimension)?
        .clone();
    let destination = destination(host, &key)
        .filter(|found| found.label != source)
        .ok_or(DimensionChangeError::NoDimension)?;
    let session = from
        .get::<Owner>(player)
        .ok_or(DimensionChangeError::NoPlayer)?
        .0;
    let anchor = placed_anchor(host, session, source).ok_or(DimensionChangeError::NotPlaced)?;
    let mut departure = depart(from, player, key, arrival)?;
    announce(host, anchor, destination, &mut departure);
    arrive(to, departure)
}
