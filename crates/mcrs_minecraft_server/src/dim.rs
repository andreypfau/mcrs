use crate::world::bus::{
    InboundConfirmMove, InboundEntitySpawn, InboundRollbackMove, OutboundPlayerPacket, PacketTarget,
};
use crate::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use crate::world::sub_app_builder::{DimLabel, DimSubAppHandle};
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::world::World;
use mcrs_minecraft_level::session::{MoveId, Place, PlayerSession, Session, SessionPlacement};
use mcrs_minecraft_level::world::in_flight::{InFlightEntry, InFlightMoves};
use mcrs_minecraft_level::world::sub_app::DimDespawnQueue;
use tracing::warn;

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

/// Whether the message was handed to the dim.
fn send_control(world: &mut World, dim_entity: Entity, msg: ToDim) -> bool {
    let Some(sender) = world
        .resource::<DimChannelsResource>()
        .get(dim_entity)
        .map(|chan| chan.control_sender.clone())
    else {
        return false;
    };
    send_control_or_teardown(&sender, dim_entity, msg, &mut world.resource_mut())
}

fn find_dim(world: &World, name: &str) -> Option<Entity> {
    world
        .try_query_filtered::<(Entity, &DimLabel), With<DimSubAppHandle>>()
        .and_then(|mut dims| {
            dims.iter(world)
                .find(|(_, label)| label.0 == name)
                .map(|(entity, _)| entity)
        })
}

/// Drain every dimension's outbox once.
///
/// Runs outside the ECS schedule, because rollback and teardown both need
/// `&mut App`, and as often as the loop idles between ticks.
pub fn pump_channels(app: &mut App) {
    let world = app.world_mut();

    let dim_entries: Vec<(Entity, Vec<FromDim>)> = {
        let Some(channels) = world.get_resource::<DimChannelsResource>() else {
            return;
        };
        channels
            .iter()
            .map(|(entity, entry)| (*entity, entry.from_dim_receiver.try_iter().collect()))
            .collect()
    };

    let mut pending_rollbacks: Vec<(MoveId, Entity, Option<PlayerSession>)> = Vec::new();
    let mut pending_confirms: Vec<(MoveId, Entity)> = Vec::new();

    for (source_dim, messages) in dim_entries {
        for message in messages {
            match message {
                FromDim::Clientbound(packet) => deliver(world, packet),
                FromDim::Spawned { move_id } => {
                    if let Some(entry) = world.resource_mut::<InFlightMoves>().remove(move_id) {
                        if let Some(session) = entry.session {
                            place_session(world, session, |placement| {
                                if let Place::Transferring { to, .. } = placement.place() {
                                    placement.set(Place::InDim(to));
                                }
                            });
                        }
                        pending_confirms.push((move_id, entry.source_dim));
                    }
                }
                FromDim::MoveEntity {
                    move_id,
                    target,
                    cause,
                    payload,
                    player: session,
                } => {
                    let Some(destination) = find_dim(world, &target) else {
                        warn!(
                            target_dim = %target,
                            "MoveEntity names an unknown dim; dropping (no rollback entry)"
                        );
                        continue;
                    };
                    let epoch = match session {
                        Some(session) => {
                            let placed = place_session(world, session, |placement| {
                                placement.set(Place::Transferring {
                                    from: source_dim,
                                    to: destination,
                                });
                            });
                            let Some(epoch) = placed else {
                                warn!(?session, "move names an unknown session; dropping");
                                continue;
                            };
                            epoch
                        }
                        None => 0,
                    };

                    // Keyed on the source-allocated id so it matches the marker
                    // the source stamped on its in-transit entity; the source
                    // sub-app owns that entity and resolves it by id itself.
                    world.resource_mut::<InFlightMoves>().insert(
                        move_id,
                        InFlightEntry {
                            source_dim,
                            session,
                            ticks_elapsed: 0,
                        },
                    );

                    let command = ToDim::SpawnEntity(InboundEntitySpawn {
                        move_id,
                        epoch,
                        cause,
                        payload,
                        player: session,
                    });
                    if !send_control(world, destination, command) {
                        world.resource_mut::<InFlightMoves>().remove(move_id);
                        pending_rollbacks.push((move_id, source_dim, session));
                    }
                }
            }
        }
    }

    for (move_id, source_dim, session) in pending_rollbacks {
        roll_back_placement(world, session);
        send_control(
            world,
            source_dim,
            ToDim::RollbackMove(InboundRollbackMove { move_id }),
        );
    }

    for (move_id, source_dim) in pending_confirms {
        send_control(
            world,
            source_dim,
            ToDim::ConfirmMove(InboundConfirmMove { move_id }),
        );
    }
}

fn deliver(world: &mut World, packet: OutboundPlayerPacket) {
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

/// Rolls back the moves no destination acknowledged in time. The timeout is
/// counted in ticks, so this runs once per tick and never from the idle loop.
pub fn expire_moves(app: &mut App) {
    let world = app.world_mut();
    let Some(mut in_flight) = world.get_resource_mut::<InFlightMoves>() else {
        return;
    };
    let timed_out = in_flight.tick_all();
    for move_id in timed_out {
        let Some(entry) = world.resource_mut::<InFlightMoves>().remove(move_id) else {
            continue;
        };
        roll_back_placement(world, entry.session);
        send_control(
            world,
            entry.source_dim,
            ToDim::RollbackMove(InboundRollbackMove { move_id }),
        );
    }
}

/// Moves are rare, so a session is found by scanning rather than through an index that
/// every login and disconnect would have to keep.
fn place_session(
    world: &mut World,
    session: PlayerSession,
    place: impl FnOnce(&mut SessionPlacement),
) -> Option<u32> {
    let mut sessions = world.query::<(&Session, &mut SessionPlacement)>();
    let (_, mut placement) = sessions.iter_mut(world).find(|(id, _)| id.0 == session)?;
    place(&mut placement);
    Some(placement.epoch())
}

fn roll_back_placement(world: &mut World, session: Option<PlayerSession>) {
    let Some(session) = session else {
        return;
    };
    place_session(world, session, |placement| {
        if let Place::Transferring { from, .. } = placement.place() {
            placement.set(Place::InDim(from));
        }
    });
}
