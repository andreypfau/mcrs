use bevy_ecs::schedule::ScheduleLabel;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::prelude::Commands;
use bevy_ecs::query::{With, Without};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Query, Res, ResMut};

/// Bridges the bus to the sockets and writes them: run in the tick's `FixedPostUpdate` and
/// again between ticks, so a packet a dimension produced off the tick leaves at once. The
/// [`FlushScope`] tells which connections it writes.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutboundFlush;

/// Which connections an [`OutboundFlush`] writes.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlushScope {
    /// Every connection, which also finds the writers that died.
    #[default]
    Every,
    /// Only the [`PendingConnections`], so a flush between ticks costs what it sends rather
    /// than one visit per connection.
    Pending,
}

/// The connections that hold what their writer has not taken yet: a packet that
/// `bridge_outbound` queued, or bytes a full writer left behind.
#[derive(Resource, Default)]
pub struct PendingConnections {
    connections: Vec<Entity>,
    visiting: Vec<Entity>,
}

pub fn run_outbound_flush(world: &mut bevy_ecs::world::World) {
    world.run_schedule(OutboundFlush);
}

/// An [`OutboundFlush`] of the [`PendingConnections`] alone. Absent without the plugin that
/// adds the flush.
pub fn run_pending_flush(world: &mut bevy_ecs::world::World) {
    *world.get_resource_or_init::<FlushScope>() = FlushScope::Pending;
    let _ = world.try_run_schedule(OutboundFlush);
    *world.get_resource_or_init::<FlushScope>() = FlushScope::Every;
}

pub fn flushes_every_connection(scope: Option<Res<FlushScope>>) -> bool {
    scope.is_none_or(|scope| *scope == FlushScope::Every)
}

pub fn flushes_pending_connections(scope: Option<Res<FlushScope>>) -> bool {
    scope.is_some_and(|scope| *scope == FlushScope::Pending)
}

use mcrs_minecraft_network::metrics::{BridgeTelemetry, PacketDispatcher};
use mcrs_minecraft_network::{ConnectionState, RawConnection, ServerSideConnection};
use mcrs_minecraft_protocol::anyhow;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundDisconnect;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerInfoUpdate;
use mcrs_minecraft_protocol_game::player_list::{PlayerListActions, PlayerListEntry};
use mcrs_minecraft_text::Text;
use std::borrow::Cow;
use tracing::{trace, warn};

use crate::bridge_queue::{DEPTH_DRAIN_TARGET, DEPTH_LIMIT, InboundRateBucket, OutboundQueue};
use crate::bus::{InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerPacket};
use crate::bus::{PacketPayload, PacketTarget};
use crate::channel_types::{DimChannelsResource, ToDim};
use crate::session::{HostAnchorRef, PendingInbound, SessionConnection};
use mcrs_minecraft_engine_world::session::{Place, Session, SessionPlacement};

/// Attach `OutboundQueue` to any connection entity that carries
/// `ServerSideConnection` but not yet an `OutboundQueue`.
///
/// Runs in `FixedPreUpdate`, ordered after `spawn_new_raw_connections`, so by
/// the time any `FixedPostUpdate` bridge system runs every connection entity
/// carries it. The network crate's spawn system cannot insert the queue because
/// it is defined in this crate; this system closes that cross-crate ownership gap.
///
/// Even with this ordering, `bridge_outbound` still treats a resolved target
/// that lacks `OutboundQueue` as a counted event
/// (`BridgeTelemetry::outbound_no_queue_total`) rather than a
/// silent miss. The counter makes any residual race observable so no join
/// packet is dropped silently.
pub fn attach_outbound_queue(
    mut commands: Commands,
    new_connections: Query<Entity, (With<ServerSideConnection>, Without<OutboundQueue>)>,
) {
    for entity in &new_connections {
        commands.entity(entity).insert(OutboundQueue::default());
    }
}

/// Drain `Messages<OutboundPlayerPacket>` once per tick, resolve each
/// `PacketTarget` against the sessions, and push packets onto the addressed
/// per-connection `OutboundQueue`.
///
/// Uses `reader.read()` (cursor semantics) so this is the single owning reader
/// of `OutboundPlayerPacket`. A second reader on the same type would produce an
/// independent cursor that re-reads from tick start — only one system may own
/// the reader.
///
/// A target that resolves to an entity with no `OutboundQueue` increments
/// `BridgeTelemetry::outbound_no_queue_total` and is never silently dropped, so
/// any residual spawn→attach race stays observable.
pub fn bridge_outbound(
    mut reader: MessageReader<OutboundPlayerPacket>,
    sessions: Query<(&Session, &SessionPlacement, Option<&SessionConnection>)>,
    mut queues: Query<&mut OutboundQueue>,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut pending: Option<ResMut<PendingConnections>>,
) {
    for msg in reader.read() {
        telemetry.outbound_messages_consumed_total += 1;

        match &msg.target {
            PacketTarget::SinglePlayer(anchor) => {
                // Session + epoch stamped at the dim boundary. PlayerSession(0) is
                // no session's id, so unstamped packets are dropped here.
                let Ok((session, placement, connection)) = sessions.get(*anchor) else {
                    continue;
                };
                if session.0 != msg.session || msg.epoch != placement.epoch() {
                    continue;
                }
                push_to_connection(
                    &mut queues,
                    &mut telemetry,
                    pending.as_deref_mut(),
                    connection,
                    msg,
                );
            }
            PacketTarget::AllInDim(dim_entity) => {
                // Broadcasts are generated for whoever is in the dim *now*, so
                // they are never stale: the per-session epoch stale-drop applies
                // only to SinglePlayer packets that may be in flight across a
                // transfer. Epoch-filtering here would wrongly drop every
                // recipient whose epoch has advanced past a broadcast's
                // unstamped epoch.
                for (_, placement, connection) in &sessions {
                    if placement.place().dim() == Some(*dim_entity) {
                        push_to_connection(
                            &mut queues,
                            &mut telemetry,
                            pending.as_deref_mut(),
                            connection,
                            msg,
                        );
                    }
                }
            }
            PacketTarget::AllPlayers => {
                // Not epoch-filtered — see AllInDim above. A fresh global
                // broadcast must reach every current session regardless of how
                // many dim transfers each has made.
                for (_, _, connection) in &sessions {
                    push_to_connection(
                        &mut queues,
                        &mut telemetry,
                        pending.as_deref_mut(),
                        connection,
                        msg,
                    );
                }
            }
            PacketTarget::PlayerSet(set) => {
                // Not epoch-filtered — see AllInDim above. The recipient set is
                // the current observer set computed this tick; each member must
                // receive it at whatever epoch they currently hold.
                for anchor in set.iter() {
                    if let Ok((_, _, connection)) = sessions.get(*anchor) {
                        push_to_connection(
                            &mut queues,
                            &mut telemetry,
                            pending.as_deref_mut(),
                            connection,
                            msg,
                        );
                    }
                }
            }
        }
    }
}

fn push_to_connection(
    queues: &mut Query<&mut OutboundQueue>,
    telemetry: &mut BridgeTelemetry,
    pending: Option<&mut PendingConnections>,
    connection: Option<&SessionConnection>,
    msg: &OutboundPlayerPacket,
) {
    let Some(connection) = connection else {
        telemetry.outbound_no_queue_total += 1;
        return;
    };
    let Ok(mut queue) = queues.get_mut(connection.entity()) else {
        telemetry.outbound_no_queue_total += 1;
        return;
    };
    // A queue is emptied whenever its connection is handed over, so one that holds packets
    // is already pending.
    if queue.total_len() == 0
        && let Some(pending) = pending
    {
        pending.connections.push(connection.entity());
    }
    queue.push(msg.clone());
}

/// Sixteen batches at the socket's cap.
const STALLED_WRITER_BYTES: usize = 16 * mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET;

/// Hands queued outbound packets to every active connection's writer, enforces the
/// drop-oldest policy, and kicks connections whose writer is dead or has stalled.
///
/// Execution order: runs in `OutboundFlush`, after `bridge_outbound` filled
/// queues; in FixedPostUpdate that flush runs before `bridge_inbound` reads.
///
/// Only the serialization of each packet runs here, in priority order; a packet encoded once
/// for many players moves as a shared buffer. Framing, compression and the socket write run on
/// each connection's writer task.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "network::dispatch_encode", skip_all)
)]
pub fn dispatch_encode(
    mut players: Query<(Entity, &mut OutboundQueue, &mut ServerSideConnection)>,
    mut commands: Commands,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut pending: Option<ResMut<PendingConnections>>,
) {
    if let Some(pending) = pending.as_deref_mut() {
        pending.connections.clear();
    }
    for (entity, mut queue, mut conn) in &mut players {
        dispatch_connection(entity, &mut queue, &mut conn, &mut commands, &mut telemetry);
        if conn.raw.unsent_bytes() > 0
            && let Some(pending) = pending.as_deref_mut()
        {
            pending.connections.push(entity);
        }
    }
}

/// [`dispatch_encode`] for the [`PendingConnections`] alone. A connection with nothing queued
/// and nothing left behind has nothing to hand over, and the tick's flush checks it for a dead
/// writer.
pub fn dispatch_pending(
    mut players: Query<(Entity, &mut OutboundQueue, &mut ServerSideConnection)>,
    mut commands: Commands,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut pending: ResMut<PendingConnections>,
) {
    let PendingConnections {
        connections,
        visiting,
    } = &mut *pending;
    std::mem::swap(connections, visiting);
    visiting.sort_unstable();
    visiting.dedup();
    for entity in visiting.drain(..) {
        let Ok((entity, mut queue, mut conn)) = players.get_mut(entity) else {
            continue;
        };
        dispatch_connection(entity, &mut queue, &mut conn, &mut commands, &mut telemetry);
        if conn.raw.unsent_bytes() > 0 {
            connections.push(entity);
        }
    }
}

fn dispatch_connection(
    entity: Entity,
    queue: &mut OutboundQueue,
    conn: &mut ServerSideConnection,
    commands: &mut Commands,
    telemetry: &mut BridgeTelemetry,
) {
    // --- (1) Disconnected writer check ---
    if conn.raw.disconnected() {
        conn.raw
            .append(&ClientboundDisconnect {
                reason: Text::from("Connection lost"),
            })
            .ok();
        conn.raw.send_written();
        warn!(conn = ?entity, "kick: the writer reported the socket dead");
        commands.entity(entity).remove::<ServerSideConnection>();
        telemetry.kick_overflow_total += 1;
        return;
    }

    // --- (1a) A writer that is behind takes what it can; one that has stalled for this
    // much is a dead socket, not a slow one ---
    if !conn.raw.flush_unsent() && conn.raw.unsent_bytes() > STALLED_WRITER_BYTES {
        warn!(
            conn = ?entity,
            unsent_bytes = conn.raw.unsent_bytes(),
            "kick: the writer has stalled"
        );
        commands.entity(entity).remove::<ServerSideConnection>();
        telemetry.kick_overflow_total += 1;
        return;
    }

    // --- (2) Drop policy: shed Normal first, then Low ---
    // Only activate if total exceeds DEPTH_LIMIT; then drain down to
    // DEPTH_DRAIN_TARGET so the queue stays below threshold for a few
    // ticks before refilling.
    if queue.total_len() > DEPTH_LIMIT {
        while queue.total_len() > DEPTH_DRAIN_TARGET {
            if queue.normal.pop_front().is_some() {
                telemetry.drop_normal_total += 1;
            } else if queue.low.pop_front().is_some() {
                telemetry.drop_low_total += 1;
            } else {
                // Only Critical/High remain; never drop them.
                break;
            }
        }
    }

    hand_over(entity, queue, &mut conn.raw, telemetry);
}

/// Serializes the queue in priority order and hands it, behind anything written to the
/// connection directly, to the writer.
fn hand_over(
    entity: Entity,
    queue: &mut OutboundQueue,
    raw: &mut RawConnection,
    telemetry: &mut BridgeTelemetry,
) {
    let OutboundQueue {
        critical,
        high,
        normal,
        low,
    } = queue;
    for sub_queue in [critical, high, normal, low] {
        for pkt in sub_queue.drain(..) {
            trace!(
                target: "mcrs_minecraft_engine::bridge",
                conn = ?entity,
                kind = ?std::mem::discriminant(&pkt.data),
                "dispatch_encode"
            );
            if let PacketPayload::Test(_) = pkt.data {
                // Test-only payload; no wire packet. Counted-drop so
                // test assertions on encode_unhandled_total work.
                telemetry.encode_unhandled_total += 1;
                continue;
            }
            if let Err(error) = append_payload(raw, pkt.data) {
                warn!(conn = ?entity, "an outbound packet did not encode: {error:#}");
            }
        }
    }
    raw.send_written();
}

fn append_payload(raw: &mut RawConnection, data: PacketPayload) -> anyhow::Result<()> {
    match data {
        PacketPayload::LightUpdate(p) => raw.append(&p),
        PacketPayload::BlockUpdate(p) => raw.append(&p),
        PacketPayload::ChunkUnload(p) => raw.append(&p),
        PacketPayload::EntityPosSync(p) => raw.append(&p),
        PacketPayload::GameEvent(p) => raw.append(&p),
        PacketPayload::PlayerLookAt(p) => raw.append(&p),
        PacketPayload::PlayerAbilities(p) => raw.append(&p),
        PacketPayload::PlayerEnteredView(p) => raw.append(&p),
        PacketPayload::ChunkLoad { packet, .. } | PacketPayload::Commands(packet) => {
            raw.append_shared(packet);
            Ok(())
        }
        PacketPayload::ChunkBatchStart(p) => raw.append(&p),
        PacketPayload::ChunkBatchFinished(p) => raw.append(&p),
        PacketPayload::PlayerLeftView(p) => raw.append(&p),
        PacketPayload::PlayerLogin(p) => raw.append(&p),
        PacketPayload::Respawn(p) => raw.append(&p),
        PacketPayload::OpLevelEntityEvent(p) => raw.append(&p),
        PacketPayload::SetChunkCacheCenter(p) => raw.append(&p),
        PacketPayload::SetChunkCacheRadius(p) => raw.append(&p),
        PacketPayload::PlayerInfoUpdate { entries } => {
            let wire_entries: Vec<PlayerListEntry<'_>> = entries
                .iter()
                .map(|e| PlayerListEntry {
                    player_uuid: e.player_uuid,
                    username: e.username.as_str(),
                    game_mode: e.game_mode,
                    listed: e.listed,
                    ..Default::default()
                })
                .collect();
            raw.append(&ClientboundPlayerInfoUpdate {
                actions: PlayerListActions::new()
                    .with_add_player(true)
                    .with_update_game_mode(true)
                    .with_update_listed(true),
                entries: Cow::Borrowed(&wire_entries),
            })
        }
        PacketPayload::PlayerPosition(p) => raw.append(&p),
        PacketPayload::SystemChat(p) => raw.append(&p),
        PacketPayload::CommandSuggestions(p) => raw.append(&p),
        PacketPayload::SetTime(p) => raw.append(&p),
        PacketPayload::Test(_) => Ok(()),
    }
}

/// Routes serverbound packets written to the bus into the dim channel seam.
///
/// `bridge_inbound` handles packets read from sockets; this system handles the
/// ones written by upstream callers, and raises their typed events on the host
/// connection as `bridge_inbound` does. A full serverbound channel disconnects the
/// offending session.
pub fn bridge_inbound_to_channel(
    mut msgs: ResMut<Messages<InboundPlayerPacket>>,
    mut sessions: Query<(
        &SessionPlacement,
        &mut PendingInbound,
        Option<&SessionConnection>,
    )>,
    dim_channels: Res<DimChannelsResource>,
    mut commands: Commands,
) {
    for msg in msgs.drain() {
        let Ok((placement, mut pending, connection)) = sessions.get_mut(msg.player) else {
            continue;
        };
        if let (Some(trigger), Some(connection)) = (msg.trigger, connection) {
            trigger.raise(
                &mut commands,
                connection.entity(),
                msg.data.clone(),
                msg.timestamp,
            );
        }
        if !route_serverbound(placement, &mut pending, &dim_channels, msg)
            && let Some(connection) = connection
        {
            commands
                .entity(connection.entity())
                .remove::<mcrs_minecraft_network::ServerSideConnection>();
        }
    }
}

/// Sends a packet to the dimension its session is attached to, or holds it
/// until the session is attached and everything held before it has gone.
/// Returns `false` when the dimension's serverbound channel is full.
fn route_serverbound(
    placement: &SessionPlacement,
    pending: &mut PendingInbound,
    dim_channels: &DimChannelsResource,
    packet: InboundPlayerPacket,
) -> bool {
    let Some(dim) = placement
        .place()
        .attached()
        .filter(|_| pending.0.is_empty())
    else {
        pending.0.push(packet);
        return true;
    };
    let Some(chan) = dim_channels.get(dim) else {
        return true;
    };
    !matches!(
        chan.serverbound_sender.try_send(ToDim::Serverbound(packet)),
        Err(flume::TrySendError::Full(_))
    )
}

/// A dimension that has spawned a player attaches its session.
pub fn bridge_player_attach(
    mut attach_msgs: ResMut<Messages<OutboundPlayerAttached>>,
    mut placements: Query<&mut SessionPlacement>,
) {
    for msg in attach_msgs.drain() {
        let Ok(mut placement) = placements.get_mut(msg.host_anchor) else {
            continue;
        };
        if let Place::Joining(dim) = placement.place() {
            placement.set(Place::InDim(dim));
        }
    }
}

/// Hands an attached session's held packets to its dimension. A full serverbound
/// channel disconnects the offending session.
pub fn forward_pending_inbound(
    mut sessions: Query<(
        &SessionPlacement,
        &mut PendingInbound,
        Option<&SessionConnection>,
    )>,
    dim_channels: Res<DimChannelsResource>,
    mut commands: Commands,
) {
    for (placement, mut pending, connection) in &mut sessions {
        if pending.0.is_empty() {
            continue;
        }
        let Some(chan) = placement
            .place()
            .attached()
            .and_then(|dim| dim_channels.get(dim))
        else {
            continue;
        };
        for packet in pending.0.drain(..) {
            if let Err(flume::TrySendError::Full(_)) =
                chan.serverbound_sender.try_send(ToDim::Serverbound(packet))
            {
                if let Some(connection) = connection {
                    commands
                        .entity(connection.entity())
                        .remove::<ServerSideConnection>();
                }
                break;
            }
        }
    }
}

pub fn bridge_inbound(
    mut conns: Query<(
        Entity,
        &mut ServerSideConnection,
        &mut InboundRateBucket,
        Option<&HostAnchorRef>,
        &ConnectionState,
    )>,
    mut commands: Commands,
    mut sessions: Query<(&SessionPlacement, &mut PendingInbound)>,
    dim_channels: Res<DimChannelsResource>,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut dispatcher: ResMut<PacketDispatcher>,
) {
    use mcrs_minecraft_text::Text;

    for (entity, mut conn, mut bucket, anchor_ref, state) in conns.iter_mut() {
        if *state != ConnectionState::Game {
            continue;
        }

        loop {
            match conn.raw.try_recv() {
                Ok(Some(pkt)) => {
                    if !bucket.consume_or_flag(pkt.timestamp) {
                        conn.disconnect(
                            ConnectionState::Game,
                            Text::from("Connection flood detected"),
                        )
                        .ok();
                        commands.entity(entity).remove::<ServerSideConnection>();
                        telemetry.kick_flood_total += 1;
                        break;
                    }

                    let trigger =
                        match dispatcher.dispatch(ConnectionState::Game, pkt.id, &pkt.payload) {
                            Ok(trigger) => trigger,
                            Err(malformed) => {
                                conn.reject(ConnectionState::Game, &malformed);
                                commands.entity(entity).remove::<ServerSideConnection>();
                                break;
                            }
                        };
                    if let Some(trigger) = trigger {
                        trigger.raise(&mut commands, entity, pkt.payload.clone(), pkt.timestamp);
                    }
                    let ends_state = dispatcher.ends_state(ConnectionState::Game, pkt.id);

                    // A row without a structure has no observer in the dimension either.
                    if trigger.is_some()
                        && let Some(&HostAnchorRef(anchor)) = anchor_ref
                        && let Ok((placement, mut pending)) = sessions.get_mut(anchor)
                        && !route_serverbound(
                            placement,
                            &mut pending,
                            &dim_channels,
                            InboundPlayerPacket {
                                player: anchor,
                                id: pkt.id,
                                data: pkt.payload,
                                timestamp: pkt.timestamp,
                                trigger,
                            },
                        )
                    {
                        commands.entity(entity).remove::<ServerSideConnection>();
                    }
                    if ends_state {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    warn!(entity = ?entity, "bridge_inbound: connection channel disconnected");
                    commands.entity(entity).remove::<ServerSideConnection>();
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;
    use bevy_ecs::system::{IntoSystem, System};
    use bevy_ecs::world::World;

    use crate::channel_types::{DimChannelsResource, FromDim, ToDim};
    use crate::session::SessionBundle;

    use bytes::Bytes;
    use mcrs_minecraft_engine_world::session::PlayerSession;
    use mcrs_minecraft_engine_world::world::channels::{
        FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
    };

    fn make_dim_channels(world: &mut World, dim: Entity) -> flume::Receiver<ToDim> {
        let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
        let (ctl_tx, _ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
        let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        world
            .resource_mut::<DimChannelsResource>()
            .insert(dim, srv_tx, ctl_tx, from_rx);
        srv_rx
    }

    fn run<M>(world: &mut World, system: impl IntoSystem<(), (), M>) {
        let mut sys = IntoSystem::into_system(system);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerAttached>>();
        world.init_resource::<Messages<InboundPlayerPacket>>();
        world.init_resource::<DimChannelsResource>();
        world
    }

    fn packet(player: Entity, id: i32) -> InboundPlayerPacket {
        InboundPlayerPacket {
            player,
            id,
            data: Bytes::new(),
            timestamp: std::time::Instant::now(),
            trigger: None,
        }
    }

    fn ids(rx: &flume::Receiver<ToDim>) -> Vec<i32> {
        rx.try_iter()
            .map(|msg| match msg {
                ToDim::Serverbound(packet) => packet.id,
                other => panic!("expected Serverbound, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn bridge_player_attach_places_the_session_and_sends_held_packets() {
        let mut world = world();
        let dest_dim = world.spawn_empty().id();
        let dest_srv_rx = make_dim_channels(&mut world, dest_dim);
        let host_anchor = world
            .spawn(SessionBundle::placed(
                PlayerSession(3),
                SessionPlacement::new(Place::Joining(dest_dim), 0),
            ))
            .id();
        world
            .get_mut::<PendingInbound>(host_anchor)
            .unwrap()
            .0
            .extend((0..3).map(|seq| packet(host_anchor, seq)));

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached { host_anchor });
        run(&mut world, bridge_player_attach);
        run(&mut world, forward_pending_inbound);

        assert_eq!(
            world.get::<SessionPlacement>(host_anchor).unwrap().place(),
            Place::InDim(dest_dim)
        );
        assert!(
            world
                .get::<PendingInbound>(host_anchor)
                .unwrap()
                .0
                .is_empty()
        );
        assert_eq!(
            ids(&dest_srv_rx),
            vec![0, 1, 2],
            "3 buffered packets sent to serverbound channel"
        );
    }

    #[test]
    fn a_packet_arriving_behind_held_ones_waits_for_them() {
        let mut world = world();
        let dim = world.spawn_empty().id();
        let srv_rx = make_dim_channels(&mut world, dim);
        let host_anchor = world
            .spawn(SessionBundle::placed(
                PlayerSession(4),
                SessionPlacement::new(Place::InDim(dim), 0),
            ))
            .id();
        world
            .get_mut::<PendingInbound>(host_anchor)
            .unwrap()
            .0
            .push(packet(host_anchor, 0));

        world
            .resource_mut::<Messages<InboundPlayerPacket>>()
            .write(packet(host_anchor, 1));
        run(&mut world, bridge_inbound_to_channel);
        assert!(ids(&srv_rx).is_empty(), "nothing overtakes a held packet");

        run(&mut world, forward_pending_inbound);
        assert_eq!(ids(&srv_rx), vec![0, 1]);
    }

    #[test]
    fn bridge_player_attach_idempotent_on_unknown_host_anchor() {
        let mut world = world();
        let unknown = Entity::from_raw_u32(999).expect("nonzero");

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached {
                host_anchor: unknown,
            });

        run(&mut world, bridge_player_attach);
        // No panic — idempotent on unknown anchor
    }
}
