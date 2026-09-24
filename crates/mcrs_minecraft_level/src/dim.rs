use crate::session::{MoveId, Place, PlayerSession, Session, SessionPlacement};
use crate::world::channels::DimChannels;
use crate::world::in_flight::{InFlightEntry, InFlightMoves};
use crate::world::sub_app::DimDespawnQueue;
use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use tracing::warn;

/// What the host must do with one message drained from a dimension's outbox.
pub enum DimRequest<P: DimProtocol + ?Sized> {
    /// Start a move: the entity is in transit in `source dim` and wants to
    /// materialise in `destination`.
    Move {
        move_id: MoveId,
        destination: Entity,
        session: Option<PlayerSession>,
        departure: P::Departure,
    },
    /// The destination acknowledged the arrival.
    Arrived { move_id: MoveId },
    /// Not part of the move protocol; handed straight back to [`DimProtocol::deliver`].
    Other(P::FromDim),
}

/// The game-shaped half of the cross-sub-app move protocol.
pub trait DimProtocol: Send + Sync + 'static {
    type ToDim: Send + Sync + 'static;
    type FromDim: Send + Sync + 'static;
    /// Whatever the destination needs to materialise the arrival. Opaque here.
    type Departure;

    /// `None` drops the message without opening a move.
    fn classify(world: &World, message: Self::FromDim) -> Option<DimRequest<Self>>;

    fn depart(
        move_id: MoveId,
        session: Option<PlayerSession>,
        epoch: u32,
        departure: Self::Departure,
    ) -> Self::ToDim;

    fn confirm(move_id: MoveId) -> Self::ToDim;

    fn roll_back(move_id: MoveId) -> Self::ToDim;

    fn deliver(world: &mut World, source_dim: Entity, message: Self::FromDim);
}

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
) {
    match sender.try_send(msg) {
        Ok(()) => {}
        Err(flume::TrySendError::Full(_)) => {
            warn!(
                dim = ?dim_entity,
                "control channel saturated (hard overload); enqueueing dim teardown"
            );
            enqueue_teardown(despawn_queue, dim_entity);
        }
        Err(flume::TrySendError::Disconnected(_)) => {}
    }
}

fn enqueue_teardown(despawn_queue: &mut DimDespawnQueue, dim_entity: Entity) {
    if !despawn_queue.0.contains(&dim_entity) {
        despawn_queue.0.push(dim_entity);
    }
}

fn send_control<P: DimProtocol>(world: &mut World, dim_entity: Entity, msg: P::ToDim) {
    let result = {
        let channels = world.resource::<DimChannels<P::ToDim, P::FromDim>>();
        channels
            .get(dim_entity)
            .map(|chan| chan.control_sender.try_send(msg))
    };
    if let Some(Err(flume::TrySendError::Full(_))) = result {
        enqueue_teardown(&mut world.resource_mut::<DimDespawnQueue>(), dim_entity);
    }
}

/// Drain every dimension's outbox once.
///
/// Runs outside the ECS schedule, because rollback and teardown both need
/// `&mut App`, and as often as the loop idles between ticks.
pub fn pump_dim_channels<P: DimProtocol>(app: &mut App) {
    let world = app.world_mut();

    let dim_entries: Vec<(Entity, Vec<P::FromDim>)> = {
        let Some(channels) = world.get_resource::<DimChannels<P::ToDim, P::FromDim>>() else {
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
            let Some(request) = P::classify(world, message) else {
                continue;
            };
            match request {
                DimRequest::Other(message) => P::deliver(world, source_dim, message),
                DimRequest::Arrived { move_id } => {
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
                DimRequest::Move {
                    move_id,
                    destination,
                    session,
                    departure,
                } => {
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

                    let command = P::depart(move_id, session, epoch, departure);
                    let sent = {
                        let channels = world.resource::<DimChannels<P::ToDim, P::FromDim>>();
                        channels
                            .get(destination)
                            .map(|chan| chan.control_sender.try_send(command))
                    };
                    match sent {
                        Some(Ok(())) => {}
                        Some(Err(flume::TrySendError::Full(_))) => {
                            world.resource_mut::<InFlightMoves>().remove(move_id);
                            pending_rollbacks.push((move_id, source_dim, session));
                            enqueue_teardown(
                                &mut world.resource_mut::<DimDespawnQueue>(),
                                destination,
                            );
                        }
                        Some(Err(flume::TrySendError::Disconnected(_))) | None => {
                            world.resource_mut::<InFlightMoves>().remove(move_id);
                            pending_rollbacks.push((move_id, source_dim, session));
                        }
                    }
                }
            }
        }
    }

    for (move_id, source_dim, session) in pending_rollbacks {
        roll_back_placement(world, session);
        send_control::<P>(world, source_dim, P::roll_back(move_id));
    }

    for (move_id, source_dim) in pending_confirms {
        send_control::<P>(world, source_dim, P::confirm(move_id));
    }
}

/// Rolls back the moves no destination acknowledged in time. The timeout is
/// counted in ticks, so this runs once per tick and never from the idle loop.
pub fn expire_moves<P: DimProtocol>(app: &mut App) {
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
        send_control::<P>(world, entry.source_dim, P::roll_back(move_id));
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
