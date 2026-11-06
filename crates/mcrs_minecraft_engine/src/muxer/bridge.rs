use bevy_ecs::schedule::ScheduleLabel;

use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::Commands;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Local, Query, Res, ResMut};
use bevy_ecs::world::World;

/// Bridges the bus to the sockets and writes every connection, which also finds the writers
/// that died: run in the tick's `FixedPostUpdate`. Between ticks, [`run_pending_flush`] writes
/// only the connections that hold something.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutboundFlush;

/// The connections that hold what their writer has not taken yet: a packet that
/// `bridge_outbound` queued, or bytes a full writer left behind.
#[derive(Resource, Default)]
pub struct PendingConnections {
    connections: Vec<Entity>,
    visiting: Vec<Entity>,
}

pub fn run_outbound_flush(world: &mut World) {
    world.run_schedule(OutboundFlush);
}

/// Bridges the bus to the sockets and writes the [`PendingConnections`] alone, so a packet a
/// dimension produced off the tick leaves at once, for the cost of what it sends. Does nothing
/// without the plugin that adds the flush.
pub fn run_pending_flush(world: &mut World) {
    if !world.contains_resource::<PendingConnections>() {
        return;
    }
    if let Err(error) = world.run_system_cached(bridge_outbound) {
        warn!("the outbound packets were not bridged: {error}");
    }
    if let Err(error) = world.run_system_cached(dispatch_pending) {
        warn!("the pending connections were not written: {error}");
    }
}

use bytes::{BufMut, Bytes, BytesMut};
use mcrs_minecraft_network::inbound_rate::InboundRateBucket;
use mcrs_minecraft_network::metrics::{BridgeTelemetry, PacketDispatcher, PacketKind};
use mcrs_minecraft_network::{
    ConnectionState, RawConnection, ServerSideConnection, SharedAppend, SharedFrames, WriterWakes,
};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundDisconnect;
use mcrs_minecraft_protocol::{Encode, Packet, WritePacket, anyhow};
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerInfoUpdate;
use mcrs_minecraft_protocol_game::player_list::{PlayerListActions, PlayerListEntry};
use mcrs_minecraft_text::Text;
use std::borrow::Cow;
use std::mem;
use tracing::{trace, warn};

use crate::bus::{InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerPacket};
use crate::bus::{PacketPayload, PacketPriority, PacketTarget};
use crate::muxer::bridge_queue::{EncodedPacket, OutboundQueue, QueuedPacket};
use crate::muxer::dim_inputs::DimInputs;
use crate::muxer::move_check::{MoveCheck, check_move};
use crate::muxer::session::{HostAnchorRef, PendingInbound, SessionConnection};
use mcrs_minecraft_engine_world::session::{Place, Session, SessionAnchors, SessionPlacement};
use mcrs_minecraft_engine_world::world::channels::MAX_HELD_SERVERBOUND;

/// Gives each new connection its `OutboundQueue`, which the network crate cannot name.
pub fn attach_outbound_queue(add: On<Add, ServerSideConnection>, mut commands: Commands) {
    commands.entity(add.entity).insert(OutboundQueue::default());
}

/// What [`bridge_outbound`] reuses from one message to the next.
#[derive(Default)]
pub struct OutboundScratch {
    recipients: Vec<Entity>,
    buffer: PacketBuffer,
}

/// Drains `Messages<OutboundPlayerPacket>`, resolves each `PacketTarget` against the sessions,
/// and pushes each packet onto the `OutboundQueue` of every connection it addresses.
///
/// A packet for more than one connection is serialized here once, and every queue holds the
/// same bytes. A packet for one connection is moved into its queue and serialized when the
/// queue is handed over.
///
/// A target that resolves to an entity with no `OutboundQueue` increments
/// `BridgeTelemetry::outbound_no_queue_total`.
pub fn bridge_outbound(
    mut messages: ResMut<Messages<OutboundPlayerPacket>>,
    anchors: Res<SessionAnchors>,
    sessions: Query<(&Session, &SessionPlacement, Option<&SessionConnection>)>,
    mut queues: Query<&mut OutboundQueue>,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut pending: ResMut<PendingConnections>,
    mut scratch: Local<OutboundScratch>,
) {
    let OutboundScratch { recipients, buffer } = &mut *scratch;
    for msg in messages.drain() {
        telemetry.outbound_messages_consumed_total += 1;
        recipients.clear();
        resolve_recipients(&msg, &anchors, &sessions, recipients, &mut telemetry);
        let Some((&last, rest)) = recipients.split_last() else {
            continue;
        };
        let packet = if rest.is_empty() {
            QueuedPacket::from(msg.data)
        } else {
            match encode_once(buffer, msg.data, &mut telemetry) {
                Ok(packet) => packet,
                Err(error) => {
                    warn!("an outbound packet did not encode: {error:#}");
                    continue;
                }
            }
        };
        for &connection in rest {
            push_to_connection(
                &mut queues,
                &mut telemetry,
                &mut pending,
                connection,
                msg.priority,
                packet.clone(),
            );
        }
        push_to_connection(
            &mut queues,
            &mut telemetry,
            &mut pending,
            last,
            msg.priority,
            packet,
        );
    }
}

/// The connections `msg` addresses. A recipient without a connection counts as
/// `outbound_no_queue_total`.
///
/// Only a packet for one player is checked against the epoch of its session, since it may be
/// in flight across a transfer. A broadcast is made for the players there now, so it is never
/// stale, and it carries no epoch to check.
fn resolve_recipients(
    msg: &OutboundPlayerPacket,
    anchors: &SessionAnchors,
    sessions: &Query<(&Session, &SessionPlacement, Option<&SessionConnection>)>,
    recipients: &mut Vec<Entity>,
    telemetry: &mut BridgeTelemetry,
) {
    let mut add = |connection: Option<&SessionConnection>| match connection {
        Some(connection) => recipients.push(connection.entity()),
        None => telemetry.outbound_no_queue_total += 1,
    };
    match &msg.target {
        PacketTarget::SinglePlayer(target) => {
            let Some(anchor) = anchors.get(*target) else {
                return;
            };
            let Ok((session, placement, connection)) = sessions.get(anchor) else {
                return;
            };
            if session.0 == msg.session && msg.epoch == placement.epoch() {
                add(connection);
            }
        }
        PacketTarget::AllInDim(dim_entity) => {
            for (_, placement, connection) in sessions {
                if placement.place().dim() == Some(*dim_entity) {
                    add(connection);
                }
            }
        }
        PacketTarget::AllPlayers => {
            for (_, _, connection) in sessions {
                add(connection);
            }
        }
        PacketTarget::PlayerSet(set) => {
            for &target in set {
                if let Some(anchor) = anchors.get(target)
                    && let Ok((_, _, connection)) = sessions.get(anchor)
                {
                    add(connection);
                }
            }
        }
    }
}

/// Serializes `payload` once for the many connections that receive it.
fn encode_once(
    buffer: &mut PacketBuffer,
    payload: PacketPayload,
    telemetry: &mut BridgeTelemetry,
) -> anyhow::Result<QueuedPacket> {
    let queued = QueuedPacket::from(payload);
    let QueuedPacket::Payload(payload) = queued else {
        return Ok(queued);
    };
    let kind = payload.kind();
    if let Err(error) = write_payload(buffer, payload) {
        buffer.0.clear();
        return Err(error);
    }
    telemetry.encode_calls_total = telemetry.encode_calls_total.saturating_add(1);
    Ok(QueuedPacket::Encoded(EncodedPacket {
        kind,
        packet: buffer.0.split().freeze(),
    }))
}

/// Packets with their ids, as `encode_with_id` writes them.
#[derive(Default)]
struct PacketBuffer(BytesMut);

impl WritePacket for PacketBuffer {
    fn write_packet_fallible<P>(&mut self, packet: &P) -> anyhow::Result<()>
    where
        P: Encode + Packet,
    {
        packet.encode_with_id((&mut self.0).writer())?;
        Ok(())
    }
}

fn push_to_connection(
    queues: &mut Query<&mut OutboundQueue>,
    telemetry: &mut BridgeTelemetry,
    pending: &mut PendingConnections,
    connection: Entity,
    priority: PacketPriority,
    packet: QueuedPacket,
) {
    let Ok(mut queue) = queues.get_mut(connection) else {
        telemetry.outbound_no_queue_total += 1;
        return;
    };
    // A queue is emptied whenever its connection is handed over, so one that holds packets
    // is already pending.
    if queue.total_len() == 0 {
        pending.connections.push(connection);
    }
    queue.push_queued(priority, packet);
}

/// The queued packets past which a connection sheds its Normal packets.
pub const DEPTH_LIMIT: usize = 256;
/// The queued packets that the shedding leaves.
pub const DEPTH_DRAIN_TARGET: usize = 192;

/// Sixteen batches at the socket's cap.
const STALLED_WRITER_BYTES: usize = 16 * mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET;

/// Hands queued outbound packets to every active connection's writer, enforces the
/// drop-oldest policy, and kicks connections whose writer is dead or has stalled.
///
/// Execution order: runs in `OutboundFlush`, after `bridge_outbound` filled
/// queues; in FixedPostUpdate that flush runs before `bridge_inbound` reads.
///
/// Only the serialization of each packet for one connection runs here, in priority order. A
/// packet serialized once for many connections is framed and compressed here once for each
/// compression threshold, and every connection sends that frame. Framing and compression of the
/// rest, and the socket write, run on each connection's writer task.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "network::dispatch_encode", skip_all)
)]
pub fn dispatch_encode(
    mut players: Query<(Entity, &mut OutboundQueue, &mut ServerSideConnection)>,
    mut commands: Commands,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut pending: ResMut<PendingConnections>,
    mut every: Local<Vec<Entity>>,
    mut frames: Local<SharedFrames>,
    mut wakes: Local<WriterWakes>,
) {
    every.extend(players.iter().map(|(entity, ..)| entity));
    pending.connections.clear();
    dispatch_listed(
        &mut players,
        &mut every,
        &mut pending.connections,
        &mut commands,
        &mut telemetry,
        &mut frames,
        &mut wakes,
    );
    every.clear();
}

/// [`dispatch_encode`] for the [`PendingConnections`] alone. A connection with nothing queued
/// and nothing left behind has nothing to hand over, and the tick's flush checks it for a dead
/// writer.
pub fn dispatch_pending(
    mut players: Query<(Entity, &mut OutboundQueue, &mut ServerSideConnection)>,
    mut commands: Commands,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut pending: ResMut<PendingConnections>,
    mut frames: Local<SharedFrames>,
    mut wakes: Local<WriterWakes>,
) {
    let PendingConnections {
        connections,
        visiting,
    } = &mut *pending;
    mem::swap(connections, visiting);
    visiting.sort_unstable();
    visiting.dedup();
    dispatch_listed(
        &mut players,
        visiting,
        connections,
        &mut commands,
        &mut telemetry,
        &mut frames,
        &mut wakes,
    );
    visiting.clear();
}

/// Hands the queued packets of each `listed` connection to its writer. A connection whose writer
/// is left with bytes it has not taken goes to `left_behind`. `listed` keeps only the connections
/// that were written.
fn dispatch_listed(
    players: &mut Query<(Entity, &mut OutboundQueue, &mut ServerSideConnection)>,
    listed: &mut Vec<Entity>,
    left_behind: &mut Vec<Entity>,
    commands: &mut Commands,
    telemetry: &mut BridgeTelemetry,
    frames: &mut SharedFrames,
    wakes: &mut WriterWakes,
) {
    // A shared packet is framed once only when more than one connection sends it, so the count
    // comes after the kicks and the shedding.
    listed.retain(|&entity| {
        let Ok((entity, mut queue, mut conn)) = players.get_mut(entity) else {
            return false;
        };
        let writable = ready_to_write(entity, &mut queue, &mut conn, commands, telemetry, wakes);
        if writable {
            count_recipients(&queue, frames);
        }
        writable
    });
    for &entity in listed.iter() {
        let Ok((entity, mut queue, mut conn)) = players.get_mut(entity) else {
            continue;
        };
        write_queue(entity, &mut queue, &mut conn.raw, telemetry, frames);
        conn.raw.send_written_deferred(wakes);
        if conn.raw.unsent_bytes() > 0 {
            left_behind.push(entity);
        }
    }
    wakes.wake_all();
    frames.clear();
}

/// Counts the connections that receive each packet serialized once, so that only a packet of
/// more than one connection costs a frame on this thread.
fn count_recipients(queue: &OutboundQueue, frames: &mut SharedFrames) {
    for sub_queue in [&queue.critical, &queue.high, &queue.normal] {
        for queued in sub_queue {
            if let QueuedPacket::Encoded(encoded) = queued {
                frames.add_recipient(&encoded.packet);
            }
        }
    }
}

/// Kicks a connection whose writer is dead or has stalled, and sheds the oldest packets of a
/// queue that is too deep. Answers whether the connection is still to be written.
fn ready_to_write(
    entity: Entity,
    queue: &mut OutboundQueue,
    conn: &mut ServerSideConnection,
    commands: &mut Commands,
    telemetry: &mut BridgeTelemetry,
    wakes: &mut WriterWakes,
) -> bool {
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
        return false;
    }

    // A writer that is behind takes what it can; one that has stalled for this much is a dead
    // socket, not a slow one.
    if !conn.raw.flush_unsent_deferred(wakes) && conn.raw.unsent_bytes() > STALLED_WRITER_BYTES {
        warn!(
            conn = ?entity,
            unsent_bytes = conn.raw.unsent_bytes(),
            "kick: the writer has stalled"
        );
        commands.entity(entity).remove::<ServerSideConnection>();
        telemetry.kick_overflow_total += 1;
        return false;
    }

    // Past DEPTH_LIMIT, drain down to DEPTH_DRAIN_TARGET so the queue stays below the limit
    // for a few ticks before it refills. Critical and High are never shed.
    if queue.total_len() > DEPTH_LIMIT {
        while queue.total_len() > DEPTH_DRAIN_TARGET && queue.normal.pop_front().is_some() {
            telemetry.drop_normal_total += 1;
        }
    }
    true
}

/// Serializes the queue in priority order, behind anything written to the connection directly.
fn write_queue(
    entity: Entity,
    queue: &mut OutboundQueue,
    raw: &mut RawConnection,
    telemetry: &mut BridgeTelemetry,
    frames: &mut SharedFrames,
) {
    let OutboundQueue {
        critical,
        high,
        normal,
    } = queue;
    for sub_queue in [critical, high, normal] {
        for queued in sub_queue.drain(..) {
            trace!(
                target: "mcrs_minecraft_engine::bridge",
                conn = ?entity,
                kind = ?queued.kind(),
                "dispatch_encode"
            );
            match queued {
                QueuedPacket::Payload(payload) => {
                    telemetry.packets.count(payload.kind());
                    if let Err(error) = write_payload(raw, payload) {
                        warn!(conn = ?entity, "an outbound packet did not encode: {error:#}");
                    } else {
                        telemetry.encode_calls_total =
                            telemetry.encode_calls_total.saturating_add(1);
                    }
                }
                QueuedPacket::Encoded(encoded) => {
                    telemetry.packets.count(encoded.kind);
                    append_encoded(entity, raw, encoded.packet, telemetry, frames);
                }
                QueuedPacket::Records(chunks) => {
                    for records in chunks {
                        telemetry.replication_bytes_total = telemetry
                            .replication_bytes_total
                            .saturating_add(records.len() as u64);
                        let packets = raw.append_records(records);
                        telemetry
                            .packets
                            .add(PacketKind::Replication, packets as u64);
                    }
                }
            }
        }
    }
}

fn append_encoded(
    entity: Entity,
    raw: &mut RawConnection,
    packet: Bytes,
    telemetry: &mut BridgeTelemetry,
    frames: &mut SharedFrames,
) {
    let bytes = packet.len() as u64;
    let append = match raw.append_shared(packet, frames) {
        Ok(append) => append,
        Err(error) => {
            warn!(conn = ?entity, "a shared packet could not be framed: {error:#}");
            return;
        }
    };
    telemetry.shared_appends_total = telemetry.shared_appends_total.saturating_add(1);
    telemetry.shared_bytes_total = telemetry.shared_bytes_total.saturating_add(bytes);
    match append {
        SharedAppend::Unframed => {}
        SharedAppend::FrameMade => {
            telemetry.shared_frames_total = telemetry.shared_frames_total.saturating_add(1);
            telemetry.framed_appends_total = telemetry.framed_appends_total.saturating_add(1);
        }
        SharedAppend::FrameReused => {
            telemetry.framed_appends_total = telemetry.framed_appends_total.saturating_add(1);
        }
    }
}

/// Writes a packet that is not serialized yet; a serialized one is an error.
fn write_payload<W: WritePacket>(sink: &mut W, data: PacketPayload) -> anyhow::Result<()> {
    match data {
        PacketPayload::LightUpdate(p) => sink.write_packet_fallible(&p),
        PacketPayload::BlockUpdate(p) => sink.write_packet_fallible(&p),
        PacketPayload::BlockEvent(p) => sink.write_packet_fallible(&p),
        PacketPayload::ChunkUnload(p) => sink.write_packet_fallible(&p),
        PacketPayload::EntityPosSync(p) => sink.write_packet_fallible(&p),
        PacketPayload::GameEvent(p) => sink.write_packet_fallible(&p),
        PacketPayload::PlayerLookAt(p) => sink.write_packet_fallible(&p),
        PacketPayload::PlayerAbilities(p) => sink.write_packet_fallible(&p),
        PacketPayload::ChunkLoad { .. }
        | PacketPayload::Commands(_)
        | PacketPayload::Replication(_) => {
            anyhow::bail!("a serialized packet reached the encoder")
        }
        PacketPayload::ChunkBatchStart(p) => sink.write_packet_fallible(&p),
        PacketPayload::ChunkBatchFinished(p) => sink.write_packet_fallible(&p),
        PacketPayload::PlayerLogin(p) => sink.write_packet_fallible(&p),
        PacketPayload::Respawn(p) => sink.write_packet_fallible(&p),
        PacketPayload::OpLevelEntityEvent(p) => sink.write_packet_fallible(&p),
        PacketPayload::SetChunkCacheCenter(p) => sink.write_packet_fallible(&p),
        PacketPayload::SetChunkCacheRadius(p) => sink.write_packet_fallible(&p),
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
            sink.write_packet_fallible(&ClientboundPlayerInfoUpdate {
                actions: PlayerListActions::new()
                    .with_add_player(true)
                    .with_update_game_mode(true)
                    .with_update_listed(true),
                entries: Cow::Borrowed(&wire_entries),
            })
        }
        PacketPayload::TabList(p) => sink.write_packet_fallible(&p),
        PacketPayload::PlayerPosition(p) => sink.write_packet_fallible(&p),
        PacketPayload::SystemChat(p) => sink.write_packet_fallible(&p),
        PacketPayload::CommandSuggestions(p) => sink.write_packet_fallible(&p),
        PacketPayload::SetTime(p) => sink.write_packet_fallible(&p),
    }
}

/// Routes serverbound packets written to the bus into the dim channel seam.
///
/// `bridge_inbound` handles packets read from sockets; this system handles the
/// ones written by upstream callers, and raises their typed events on the host
/// connection as `bridge_inbound` does. A session that holds more than
/// [`MAX_HELD_SERVERBOUND`] packets is disconnected.
pub fn bridge_inbound_to_channel(
    mut msgs: ResMut<Messages<InboundPlayerPacket>>,
    anchors: Res<SessionAnchors>,
    mut sessions: Query<(
        &SessionPlacement,
        &mut PendingInbound,
        Option<&SessionConnection>,
    )>,
    mut dim_inputs: ResMut<DimInputs>,
    mut commands: Commands,
) {
    for msg in msgs.drain() {
        let Some(anchor) = anchors.get(msg.session) else {
            continue;
        };
        let Ok((placement, mut pending, connection)) = sessions.get_mut(anchor) else {
            continue;
        };
        if let (Some(trigger), Some(connection)) = (msg.trigger, connection) {
            trigger.raise(&mut commands, connection.entity(), msg.data.clone());
        }
        if !route_serverbound(placement, &mut pending, &mut dim_inputs, msg)
            && let Some(connection) = connection
        {
            commands
                .entity(connection.entity())
                .remove::<mcrs_minecraft_network::ServerSideConnection>();
        }
    }
}

/// Sends a packet to the dimension its session is attached to, or holds it
/// until the session is attached, the next tick has room, and everything held
/// before it has gone. Returns `false` when the session holds more than
/// [`MAX_HELD_SERVERBOUND`] packets for its attached dimension.
fn route_serverbound(
    placement: &SessionPlacement,
    pending: &mut PendingInbound,
    dim_inputs: &mut DimInputs,
    packet: InboundPlayerPacket,
) -> bool {
    let Some(dim) = placement.place().attached() else {
        pending.0.push(packet);
        return true;
    };
    if !pending.0.is_empty() {
        pending.0.push(packet);
        return pending.0.len() <= MAX_HELD_SERVERBOUND;
    }
    if !dim_inputs.contains(dim) {
        return true;
    }
    if let Err(packet) = dim_inputs.try_send_serverbound(dim, packet) {
        pending.0.push(packet);
    }
    true
}

/// A dimension that has spawned a player attaches its session.
pub fn bridge_player_attach(
    mut attach_msgs: ResMut<Messages<OutboundPlayerAttached>>,
    anchors: Res<SessionAnchors>,
    mut placements: Query<&mut SessionPlacement>,
) {
    for msg in attach_msgs.drain() {
        let Some(anchor) = anchors.get(msg.session) else {
            continue;
        };
        let Ok(mut placement) = placements.get_mut(anchor) else {
            continue;
        };
        if let Place::Joining(dim) = placement.place() {
            placement.set(Place::InDim(dim));
        }
    }
}

/// Hands an attached session's held packets to its dimension, oldest first, until the
/// serverbound packets of the dimension's next tick are full; the rest wait for the next call.
pub fn forward_pending_inbound(
    mut sessions: Query<(&SessionPlacement, &mut PendingInbound)>,
    mut dim_inputs: ResMut<DimInputs>,
) {
    for (placement, mut pending) in &mut sessions {
        if pending.0.is_empty() {
            continue;
        }
        let Some(dim) = placement
            .place()
            .attached()
            .filter(|&dim| dim_inputs.contains(dim))
        else {
            continue;
        };
        let mut held = mem::take(&mut pending.0).into_iter();
        while let Some(packet) = held.next() {
            if let Err(packet) = dim_inputs.try_send_serverbound(dim, packet) {
                pending.0.push(packet);
                pending.0.extend(held);
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
    mut sessions: Query<(&Session, &SessionPlacement, &mut PendingInbound)>,
    mut dim_inputs: ResMut<DimInputs>,
    mut telemetry: ResMut<BridgeTelemetry>,
    mut dispatcher: ResMut<PacketDispatcher>,
) {
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
                    if check_move(pkt.id, &pkt.payload) == MoveCheck::NotFinite {
                        conn.disconnect(ConnectionState::Game, MoveCheck::reason())
                            .ok();
                        commands.entity(entity).remove::<ServerSideConnection>();
                        break;
                    }
                    if let Some(trigger) = trigger {
                        trigger.raise_arrived(
                            &mut commands,
                            entity,
                            pkt.payload.clone(),
                            pkt.timestamp,
                        );
                    }
                    let ends_state = dispatcher.ends_state(ConnectionState::Game, pkt.id);

                    // A row without a structure has no observer in the dimension either.
                    if trigger.is_some()
                        && let Some(&HostAnchorRef(anchor)) = anchor_ref
                        && let Ok((&Session(session), placement, mut pending)) =
                            sessions.get_mut(anchor)
                        && !route_serverbound(
                            placement,
                            &mut pending,
                            &mut dim_inputs,
                            InboundPlayerPacket {
                                session,
                                id: pkt.id,
                                data: pkt.payload,
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

    use crate::muxer::session::SessionBundle;

    use bytes::Bytes;
    use mcrs_minecraft_engine_world::session::PlayerSession;

    fn spawn_dim(world: &mut World) -> Entity {
        let dim = world.spawn_empty().id();
        world.resource_mut::<DimInputs>().insert(dim);
        dim
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
        world.init_resource::<DimInputs>();
        world.init_resource::<SessionAnchors>();
        world
    }

    fn packet(session: PlayerSession, id: i32) -> InboundPlayerPacket {
        InboundPlayerPacket {
            session,
            id,
            data: Bytes::new(),
            trigger: None,
        }
    }

    fn ids(world: &World, dim: Entity) -> Vec<i32> {
        world
            .resource::<DimInputs>()
            .serverbound(dim)
            .iter()
            .map(|packet| packet.id)
            .collect()
    }

    #[test]
    fn bridge_player_attach_places_the_session_and_sends_held_packets() {
        let mut world = world();
        let dest_dim = spawn_dim(&mut world);
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
            .extend((0..3).map(|seq| packet(PlayerSession(3), seq)));

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached {
                session: PlayerSession(3),
            });
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
            ids(&world, dest_dim),
            vec![0, 1, 2],
            "the 3 held packets go to the next tick of the dimension"
        );
    }

    #[test]
    fn a_packet_arriving_behind_held_ones_waits_for_them() {
        let mut world = world();
        let dim = spawn_dim(&mut world);
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
            .push(packet(PlayerSession(4), 0));

        world
            .resource_mut::<Messages<InboundPlayerPacket>>()
            .write(packet(PlayerSession(4), 1));
        run(&mut world, bridge_inbound_to_channel);
        assert!(
            ids(&world, dim).is_empty(),
            "nothing overtakes a held packet"
        );

        run(&mut world, forward_pending_inbound);
        assert_eq!(ids(&world, dim), vec![0, 1]);
    }

    #[test]
    fn bridge_player_attach_idempotent_on_unknown_session() {
        let mut world = world();

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached {
                session: PlayerSession(999),
            });

        run(&mut world, bridge_player_attach);
        // No panic — idempotent on unknown session
    }
}
