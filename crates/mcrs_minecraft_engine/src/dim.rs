use crate::bus::{OutboundPlayerAttached, OutboundPlayerPacket, PacketTarget};
use crate::dim_cells::{DimCells, DimTicked};
use crate::dim_inputs::DimInputs;
use crate::disconnect::Departing;
use crate::player_dimension_change;
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::session::{
    PlayerSession, Session, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::lifecycle::trace::ColumnTraceSink;
use mcrs_minecraft_engine_world::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_resource::ResourceKeyBuf;
use tracing::{debug, warn};

/// What the host took from the dimensions and has not handled yet, kept from one take to the
/// next for its capacity.
#[derive(Resource, Default)]
struct TakenOutputs(Vec<DimTicked>);

/// Takes what the dimension worlds handed over in their last tick, the dimensions in the order
/// they ticked: stamps and writes their packets to the host, forgets the players they released,
/// attaches the sessions they spawned a player for and records their column traces.
pub fn take_dim_outputs(world: &mut World) {
    let mut taken = world.remove_resource::<TakenOutputs>().unwrap_or_default();
    if let Some(mut cells) = world.get_resource_mut::<DimCells>() {
        cells.take_outputs(&mut taken.0);
    }
    for DimTicked { label, output } in taken.0.drain(..) {
        for packet in output.packets {
            deliver(world, packet);
        }
        for session in output.released {
            forget_departures(world, |departing| {
                departing.dim == label && departing.session == session
            });
        }
        if !output.attached.is_empty()
            && let Some(mut attached) = world.get_resource_mut::<Messages<OutboundPlayerAttached>>()
        {
            attached.write_batch(output.attached);
        }
        if !output.traces.is_empty()
            && let Some(sink) = world.get_resource::<ColumnTraceSink>()
            && let Some(dimension) = world.get::<ResourceKeyBuf<DimensionKey>>(label)
        {
            sink.record(dimension.as_str(), output.traces);
        }
        for departure in output.departures {
            player_dimension_change::route_departure(world, label, departure);
        }
    }
    world.insert_resource(taken);
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

/// Marks the host entity that labels a dimension world. The entity carries the key of the
/// dimension and no other state: it gives the dimension a host-unique name.
#[derive(bevy_ecs::component::Component)]
pub struct DimSubAppHandle;

/// Stops the dimension worlds of the `DimDespawnQueue` and despawns their label entities.
pub fn drain_dim_despawn_queue(app: &mut App) {
    let Some(mut queue) = app.world_mut().get_resource_mut::<DimDespawnQueue>() else {
        return;
    };
    let entities: Vec<Entity> = std::mem::take(&mut queue.0);
    for entity in entities {
        let world = app.world_mut();
        let removed = world
            .get_resource_mut::<DimCells>()
            .is_some_and(|mut cells| cells.remove(entity));
        if !removed {
            warn!(?entity, "DimDespawnQueue entry names no dimension world");
        }
        if let Some(mut inputs) = world.get_resource_mut::<DimInputs>() {
            inputs.remove(entity);
        }
        // A dimension torn down saves none of its players, so nothing is left to wait for.
        forget_departures(world, |departing| departing.dim == entity);

        // The `OnRemove<DimSubAppHandle>` observer queues an entity that is despawned already.
        match world.get_entity_mut(entity) {
            Ok(entity_mut) => entity_mut.despawn(),
            Err(_) => debug!(
                ?entity,
                "DimDespawnQueue entity already absent from host world (expected on the OnRemove observer path)"
            ),
        }
    }
}
