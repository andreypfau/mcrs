use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::session::{
    PlayerSession, Session, SessionAnchors, SessionPlacement,
};

use crate::bus::{OutboundPlayerAttached, OutboundPlayerPacket, PacketTarget};
use crate::muxer::departure_routing;
use crate::muxer::disconnect::Departing;
use crate::muxer::entity_replication::{self, Answers};
use crate::to_muxer::DimDelivered;

/// Takes what the dimensions handed over in their last tick, in the order they ticked: stamps
/// and writes their packets to the host, forgets the players they released, attaches the
/// sessions they spawned a player for, hands their tile deltas to the entity muxer and routes
/// the players that left them. The entity bytes that the entity muxer has ready go out with the
/// packets.
pub(crate) fn receive(world: &mut World) {
    let delivered: Vec<DimDelivered> = world
        .get_resource_mut::<Messages<DimDelivered>>()
        .map(|mut messages| messages.drain().collect())
        .unwrap_or_default();
    for DimDelivered {
        dim,
        packets,
        released,
        attached,
        tiles,
        departures,
    } in delivered
    {
        for packet in packets {
            deliver(world, packet);
        }
        for session in released {
            forget_departures(world, |departing| {
                departing.dim == dim && departing.session == session
            });
        }
        if !attached.is_empty()
            && let Some(mut messages) = world.get_resource_mut::<Messages<OutboundPlayerAttached>>()
        {
            messages.write_batch(attached);
        }
        entity_replication::publish(world, dim, tiles);
        for departure in departures {
            departure_routing::route_departure(world, dim, departure);
        }
    }
    entity_replication::deliver(world, Answers::Ready);
}

pub fn forget_departures(world: &mut World, matches: impl Fn(&Departing) -> bool) {
    let released: Vec<Entity> = world
        .query::<(Entity, &Departing)>()
        .iter(world)
        .filter(|(_, departing)| matches(departing))
        .map(|(entity, _)| entity)
        .collect();
    for entity in released {
        world.despawn(entity);
    }
}

/// Stamps `packet` with the session and the epoch of its player and writes it to the host's
/// outbound packets.
pub fn deliver(world: &mut World, packet: OutboundPlayerPacket) {
    let (session, epoch) = match &packet.target {
        PacketTarget::SinglePlayer(target) => world
            .get_resource::<SessionAnchors>()
            .and_then(|anchors| anchors.get(*target))
            .and_then(|anchor| world.get_entity(anchor).ok())
            .and_then(|anchor| {
                Some((
                    anchor.get::<Session>()?.0,
                    anchor.get::<SessionPlacement>()?.epoch(),
                ))
            })
            .unwrap_or((PlayerSession(0), 0)),
        _ => (PlayerSession(0), 0),
    };

    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            session,
            epoch,
            ..packet
        });
}
