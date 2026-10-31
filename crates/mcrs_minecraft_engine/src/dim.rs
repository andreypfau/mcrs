use crate::bus::{OutboundPlayerPacket, PacketTarget};
use crate::channel_types::{DimChannelsResource, FromDim, ToDim};
use crate::disconnect::Departing;
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::session::{PlayerSession, Session, SessionPlacement};
use mcrs_minecraft_engine_world::world::channels::DimChannels;
use mcrs_minecraft_engine_world::world::sub_app::{DimAppLabel, DimDespawnQueue};
use tracing::{debug, warn};

/// Send a non-sheddable control message, escalating a saturated control channel
/// to whole-dim teardown.
///
/// The control channel holds the lifecycle reserve that the two-channel split
/// guarantees is always available under normal load. A `Full` here therefore
/// means the dim has stopped draining its inbox — hard overload — so its label
/// `Entity` is enqueued for teardown rather than silently dropping the message.
/// A `Disconnected` channel means the dim is already gone.
pub fn send_control_or_teardown<T>(
    sender: &flume::Sender<T>,
    dim_entity: Entity,
    msg: T,
    despawn_queue: &mut DimDespawnQueue,
) -> bool {
    match sender.try_send(msg) {
        Ok(()) => true,
        Err(flume::TrySendError::Full(_)) => {
            warn!(
                dim = ?dim_entity,
                "control channel saturated (hard overload); enqueueing dim teardown"
            );
            enqueue_teardown(despawn_queue, dim_entity);
            false
        }
        Err(flume::TrySendError::Disconnected(_)) => false,
    }
}

fn enqueue_teardown(despawn_queue: &mut DimDespawnQueue, dim_entity: Entity) {
    if !despawn_queue.0.contains(&dim_entity) {
        despawn_queue.0.push(dim_entity);
    }
}

#[derive(Resource, Default)]
struct PumpScratch {
    order: Vec<Entity>,
    messages: Vec<FromDim>,
}

fn with_pump_scratch(world: &mut World, pump: impl FnOnce(&mut World, &mut PumpScratch)) {
    let mut scratch = core::mem::take(&mut *world.get_resource_or_init::<PumpScratch>());
    pump(world, &mut scratch);
    *world.get_resource_or_init::<PumpScratch>() = scratch;
}

/// Drain every dimension's outbox once, the dimensions in the order of their entities.
///
/// Runs outside the ECS schedule, as often as the loop idles between ticks.
pub fn pump_channels(app: &mut App) {
    with_pump_scratch(app.world_mut(), |world, scratch| {
        scratch.order.clear();
        if let Some(channels) = world.get_resource::<DimChannelsResource>() {
            scratch.order.extend(channels.iter().map(|(dim, _)| *dim));
        }
        scratch.order.sort_unstable();
        for &dim in &scratch.order {
            drain_dim(world, dim, &mut scratch.messages);
        }
    });
}

/// Takes what the dimension `dim` already put in its channel, as [`pump_channels`] does for
/// every dimension.
pub fn pump_dim(world: &mut World, dim: Entity) {
    with_pump_scratch(world, |world, scratch| {
        drain_dim(world, dim, &mut scratch.messages);
    });
}

fn drain_dim(world: &mut World, dim: Entity, messages: &mut Vec<FromDim>) {
    if let Some(entry) = world
        .get_resource::<DimChannelsResource>()
        .and_then(|channels| channels.get(dim))
    {
        let waiting = entry.from_dim_receiver.len();
        messages.extend(entry.from_dim_receiver.try_iter().take(waiting));
    }
    for message in messages.drain(..) {
        receive_from_dim(world, dim, message);
    }
}

/// Handles one message that the dimension `source_dim` sent to the host.
pub fn receive_from_dim(world: &mut World, source_dim: Entity, message: FromDim) {
    match message {
        FromDim::Clientbound(packet) => deliver(world, packet),
        FromDim::Released { session } => {
            forget_departures(world, |departing| {
                departing.dim == source_dim && departing.session == session
            });
        }
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
pub fn deliver(world: &mut World, packet: OutboundPlayerPacket) {
    let (session, epoch) = match &packet.target {
        PacketTarget::SinglePlayer(anchor) => world
            .get_entity(*anchor)
            .ok()
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

/// Marker component placed on the host-world entity that anchors a
/// `DimAppLabel`. The entity carries no other state — it exists purely to
/// allocate a `World`-unique ID for use as the sub-app label key.
#[derive(bevy_ecs::component::Component)]
pub struct DimSubAppHandle;

/// Drain the `DimDespawnQueue` resource on the host world and tear down the
/// matching sub-apps. Called from outside the ECS run loop because
/// `App::remove_sub_app` requires `&mut App`.
pub fn drain_dim_despawn_queue(app: &mut App) {
    let Some(mut queue) = app.world_mut().get_resource_mut::<DimDespawnQueue>() else {
        return;
    };
    let entities: Vec<Entity> = std::mem::take(&mut queue.0);
    for entity in entities {
        if app.remove_sub_app(DimAppLabel(entity)).is_none() {
            warn!(
                ?entity,
                "DimDespawnQueue entry referenced a sub-app not registered under DimAppLabel"
            );
        }

        // Remove the channel entry for the despawned dim. After the sub-app
        // is gone its dim-side FromDimSender is dropped, so the host-side
        // receiver would return Disconnected on any subsequent drain attempt.
        // Removing the entry here keeps DimChannels consistent with the live
        // sub-app population and avoids holding a stale receiver.
        if let Some(mut channels) = app
            .world_mut()
            .get_resource_mut::<DimChannels<ToDim, FromDim>>()
        {
            channels.remove(entity);
        }
        // A dimension torn down saves none of its players, so nothing is left to wait for.
        forget_departures(app.world_mut(), |departing| departing.dim == entity);

        // Free the host-side label-anchor entity so the host world's
        // dimension-handle archetype matches the live sub-app population.
        // The OnRemove<DimSubAppHandle> observer fires before this drain runs
        // (it fires at despawn time), so when the observer path is active the
        // entity is already gone here — Err(_) is expected on that path.
        match app.world_mut().get_entity_mut(entity) {
            Ok(entity_mut) => entity_mut.despawn(),
            Err(_) => debug!(
                ?entity,
                "DimDespawnQueue entity already absent from host world (expected on the OnRemove observer path)"
            ),
        }
    }
}
