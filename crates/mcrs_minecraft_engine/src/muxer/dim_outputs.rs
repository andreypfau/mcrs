use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::session::{
    PlayerSession, Session, SessionAnchors, SessionPlacement,
};

use crate::bus::{OutboundPlayerAttached, OutboundPlayerPacket, PacketTarget};
use crate::channel_types::ToDim;
use crate::muxer::column_view_link;
use crate::muxer::column_views::DimColumns;
use crate::muxer::departure_routing;
use crate::muxer::dim_inputs::DimInputs;
use crate::muxer::disconnect::Departing;
use crate::muxer::entity_replication::{self, Answers};
use crate::player_order::OrderedPlayer;
use crate::to_muxer::DimDelivered;

/// Takes what the dimensions handed over in their last tick, in the order they ticked: stamps
/// and writes their packets to the host, forgets the players they released, attaches the
/// sessions they spawned a player for, hands their tile deltas to the entity muxer and routes
/// the players that left them. Then it routes the orders for players to the dimension that holds
/// each player, and the command runs that go on elsewhere to their dimension. The entity bytes
/// that the entity muxer has ready go out with the packets.
pub(crate) fn receive(world: &mut World) {
    let delivered: Vec<DimDelivered> = world
        .get_resource_mut::<Messages<DimDelivered>>()
        .map(|mut messages| messages.drain().collect())
        .unwrap_or_default();
    // A departure places its session in another dimension, and a packet of the views is
    // stamped with the placement it is delivered under: what the views sent for the old
    // placement goes out first.
    let departs = delivered
        .iter()
        .any(|delivered| !delivered.departures.is_empty());
    column_view_link::deliver(
        world,
        if departs {
            Answers::All
        } else {
            Answers::Ready
        },
    );
    let mut orders = Vec::new();
    let mut handoffs = Vec::new();
    for DimDelivered {
        dim,
        packets,
        released,
        attached,
        tiles,
        departures,
        orders: ordered,
        handoffs: handed_off,
        views,
        column_versions,
        column_changes,
    } in delivered
    {
        // Before the departures: a view that the dimension closed sends nothing after a respawn.
        column_view_link::receive(
            world,
            dim,
            DimColumns {
                views,
                versions: column_versions,
                changes: column_changes,
            },
        );
        orders.extend(ordered);
        handoffs.extend(handed_off);
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
    for order in orders {
        route_order(world, order);
    }
    for handoff in handoffs {
        let to = handoff.to;
        let sent = world
            .get_resource_mut::<DimInputs>()
            .is_some_and(|mut inputs| inputs.send_control(to, ToDim::Command(Box::new(handoff))));
        if !sent {
            tracing::error!(dim = ?to, "a command run goes on in a dimension that takes no input");
        }
    }
    column_view_link::send(world);
    column_view_link::deliver(world, Answers::Ready);
    entity_replication::deliver(world, Answers::Ready);
}

/// Sends `order` to the dimension where the session of its player is placed.
fn route_order(world: &mut World, order: OrderedPlayer) {
    let session = order.session;
    let dim = world
        .get_resource::<SessionAnchors>()
        .and_then(|anchors| anchors.get(session))
        .and_then(|anchor| world.get::<SessionPlacement>(anchor))
        .and_then(|placement| placement.place().dim());
    let sent = dim.is_some_and(|dim| {
        world
            .get_resource_mut::<DimInputs>()
            .is_some_and(|mut inputs| inputs.send_control(dim, ToDim::Order(order)))
    });
    if !sent {
        tracing::error!(?session, "an order names a player that no dimension holds");
    }
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
pub fn deliver(world: &mut World, mut packet: OutboundPlayerPacket) {
    Stamp::of(world, &packet.target).apply(&mut packet);
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(packet);
}

/// As [`deliver`] for each of `packets`. The packets of one player come in a run, so a run
/// costs one look up of its player.
pub fn deliver_all(world: &mut World, mut packets: Vec<OutboundPlayerPacket>) {
    let mut last: Option<SessionStamp> = None;
    for packet in &mut packets {
        let stamp = match (&packet.target, last) {
            (PacketTarget::SinglePlayer(target), Some(last)) if *target == last.target => {
                last.stamp
            }
            (PacketTarget::SinglePlayer(target), _) => {
                let stamp = Stamp::of(world, &packet.target);
                last = Some(SessionStamp {
                    target: *target,
                    stamp,
                });
                stamp
            }
            _ => Stamp::NONE,
        };
        stamp.apply(packet);
    }
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write_batch(packets);
}

/// The session and the epoch of the player that a packet addresses.
#[derive(Clone, Copy, Debug)]
struct Stamp {
    session: PlayerSession,
    epoch: u32,
}

/// The stamp of the packets for `target`.
#[derive(Clone, Copy, Debug)]
struct SessionStamp {
    target: PlayerSession,
    stamp: Stamp,
}

impl Stamp {
    /// No session: the bridge sends such a packet for one player to no one.
    const NONE: Self = Self {
        session: PlayerSession(0),
        epoch: 0,
    };

    /// A packet for one player is stamped with its session, and any other packet with none.
    fn of(world: &World, target: &PacketTarget) -> Self {
        let PacketTarget::SinglePlayer(target) = target else {
            return Self::NONE;
        };
        world
            .get_resource::<SessionAnchors>()
            .and_then(|anchors| anchors.get(*target))
            .and_then(|anchor| world.get_entity(anchor).ok())
            .and_then(|anchor| {
                Some(Self {
                    session: anchor.get::<Session>()?.0,
                    epoch: anchor.get::<SessionPlacement>()?.epoch(),
                })
            })
            .unwrap_or(Self::NONE)
    }

    fn apply(self, packet: &mut OutboundPlayerPacket) {
        packet.session = self.session;
        packet.epoch = self.epoch;
    }
}
