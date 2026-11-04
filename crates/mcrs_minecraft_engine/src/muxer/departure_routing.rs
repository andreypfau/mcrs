use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_protocol_game::clientbound::ClientboundRespawn;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;

use crate::bus::{self, PacketPayload};
use crate::channel_types::ToDim;
use crate::departure::{Departure, PlayerTransfer};
use crate::enter_level_packets::enter_level_packets;
use crate::game_mode;
use crate::muxer::bridge::OutboundFlush;
use crate::muxer::dim_inputs::DimInputs;
use crate::muxer::dim_outputs;
use crate::muxer::entity_replication::{self, Answers};
use crate::to_muxer::{Destination, RoutedDeparture};

/// Both bits of the data to keep: the attribute modifiers and the entity data.
const KEEP_ATTRIBUTES_AND_DATA: u8 = 0x03;

/// The host anchor of `session` while the session is placed in the dimension `source`.
fn placed_anchor(host: &World, session: PlayerSession, source: Entity) -> Option<Entity> {
    host.get_resource::<SessionAnchors>()
        .and_then(|anchors| anchors.get(session))
        .filter(|&anchor| {
            host.get::<SessionPlacement>(anchor)
                .is_some_and(|placement| placement.place().dim() == Some(source))
        })
}

/// Sends the departure of `routed`, which the dimension `source` handed over, on to its
/// destination for its next tick, and sends the client the packets of a respawn. A departure
/// without a destination, or whose session is no longer placed in `source`, goes back to
/// `source`.
pub fn route_departure(host: &mut World, source: Entity, routed: RoutedDeparture) {
    let RoutedDeparture {
        mut departure,
        destination,
    } = routed;
    let anchor = placed_anchor(host, departure.session, source);
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
    // or the bridge drops them as stale. The muxer bytes of that dimension are among them, and
    // the muxer may still work on them, so the host waits for it here.
    entity_replication::leave(host, departure.session);
    entity_replication::deliver(host, Answers::All);
    if let Err(error) = host.try_run_schedule(OutboundFlush) {
        tracing::warn!("the packets of a player leaving were not flushed: {error}");
    }
    if let Some(mut placement) = host.get_mut::<SessionPlacement>(anchor) {
        placement.set(Place::InDim(destination.label));
    }
    let session = departure.session;
    let carried = &mut departure.carried;
    let teleport_id = carried.teleport.sent(departure.arrival.transform);
    dim_outputs::deliver(
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
        |packet| dim_outputs::deliver(host, packet),
    );
    if let Some(look_at) = departure.arrival.look_at.clone() {
        dim_outputs::deliver(
            host,
            bus::to(session, PacketPayload::PlayerLookAt(look_at)).critical(),
        );
    }
}
