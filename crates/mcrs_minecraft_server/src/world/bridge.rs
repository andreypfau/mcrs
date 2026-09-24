use bevy_ecs::schedule::ScheduleLabel;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::prelude::Commands;
use bevy_ecs::query::{With, Without};
use bevy_ecs::system::{Query, Res, ResMut};

/// Bridges the bus to the sockets and writes them: run in the tick's `FixedPostUpdate` and
/// again between ticks, so a packet a dimension produced off the tick leaves at once.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutboundFlush;

pub fn run_outbound_flush(world: &mut bevy_ecs::world::World) {
    world.run_schedule(OutboundFlush);
}

use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_network::metrics::BridgeTelemetry;
use mcrs_minecraft_network::{ConnectionState, ServerSideConnection};
use mcrs_minecraft_protocol::Text;
use mcrs_minecraft_protocol::chunk::ChunkData;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundDisconnect, ClientboundLevelChunkWithLight, ClientboundPlayerInfoUpdate,
};
use mcrs_minecraft_protocol::profile::{PlayerListActions, PlayerListEntry};
use std::borrow::Cow;
use tracing::{trace, warn};

use crate::world::bridge_queue::{
    DEPTH_DRAIN_TARGET, DEPTH_LIMIT, InboundRateBucket, OutboundQueue,
};
use crate::world::bus::{InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerPacket};
use crate::world::bus::{PacketPayload, PacketTarget};
use crate::world::channel_types::{DimChannelsResource, ToDim};
use crate::world::session::{HostAnchorRef, PendingInbound, SessionConnection};
use mcrs_minecraft_level::session::{Place, Session, SessionPlacement};

/// Attach `OutboundQueue` and `InboundRateBucket` to any connection entity that
/// carries `ServerSideConnection` but not yet an `OutboundQueue`.
///
/// Runs in `FixedPreUpdate`, ordered after `spawn_new_raw_connections`, so by
/// the time any `FixedPostUpdate` bridge system runs every connection entity
/// carries both components. The network crate's spawn system cannot insert
/// these components because they are defined in this crate; this system closes
/// that cross-crate ownership gap.
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
        commands
            .entity(entity)
            .insert((OutboundQueue::default(), InboundRateBucket::new()));
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
                push_to_connection(&mut queues, &mut telemetry, connection, msg);
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
                        push_to_connection(&mut queues, &mut telemetry, connection, msg);
                    }
                }
            }
            PacketTarget::AllPlayers => {
                // Not epoch-filtered — see AllInDim above. A fresh global
                // broadcast must reach every current session regardless of how
                // many dim transfers each has made.
                for (_, _, connection) in &sessions {
                    push_to_connection(&mut queues, &mut telemetry, connection, msg);
                }
            }
            PacketTarget::PlayerSet(set) => {
                // Not epoch-filtered — see AllInDim above. The recipient set is
                // the current observer set computed this tick; each member must
                // receive it at whatever epoch they currently hold.
                for anchor in set.iter() {
                    if let Ok((_, _, connection)) = sessions.get(*anchor) {
                        push_to_connection(&mut queues, &mut telemetry, connection, msg);
                    }
                }
            }
        }
    }
}

fn push_to_connection(
    queues: &mut Query<&mut OutboundQueue>,
    telemetry: &mut BridgeTelemetry,
    connection: Option<&SessionConnection>,
    msg: &OutboundPlayerPacket,
) {
    match connection.map(|connection| queues.get_mut(connection.entity())) {
        Some(Ok(mut queue)) => queue.push(msg.clone()),
        _ => telemetry.outbound_no_queue_total += 1,
    }
}

const STALLED_WRITER_BYTES: usize = 16 * mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET;

/// Encode queued outbound packets for every active connection, enforce the
/// drop-oldest policy, kick connections that overflow Critical/High backlogs,
/// and coalesce all encoded bytes into a single `try_send_blob` per socket per
/// tick.
///
/// Execution order: runs in `OutboundFlush`, after `bridge_outbound` filled
/// queues; in FixedPostUpdate that flush runs before `bridge_inbound` reads.
///
/// SEQUENTIAL `iter_mut()` — do NOT use `par_iter_mut`. Kicking a connection
/// issues `commands.entity(e).remove::<ServerSideConnection>()`, which
/// requires exclusive Commands access not safe across parallel workers.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "network::dispatch_encode", skip_all)
)]
/// Sixteen blobs at the socket's cap.

pub fn dispatch_encode(
    mut players: Query<(Entity, &mut OutboundQueue, &mut ServerSideConnection)>,
    mut commands: Commands,
    mut telemetry: ResMut<BridgeTelemetry>,
) {
    use mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET;

    for (entity, mut queue, mut conn) in players.iter_mut() {
        // --- (1) Disconnected writer check (AP-06 path) ---
        if conn.raw.disconnected() {
            conn.raw
                .append(&ClientboundDisconnect {
                    reason: Text::from("Connection lost"),
                })
                .ok();
            let blob = conn.raw.take_encoded();
            conn.raw.try_send_blob(blob);
            warn!(conn = ?entity, "kick: the writer reported the socket dead");
            commands.entity(entity).remove::<ServerSideConnection>();
            telemetry.kick_overflow_total += 1;
            continue;
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
            continue;
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

        // --- (3) Encode survivors in priority order ---
        let encode_queues = [
            std::mem::take(&mut queue.critical),
            std::mem::take(&mut queue.high),
            std::mem::take(&mut queue.normal),
            std::mem::take(&mut queue.low),
        ];

        for sub_queue in encode_queues {
            for pkt in sub_queue {
                trace!(
                    target: "mcrs_minecraft_server::bridge",
                    conn = ?entity,
                    kind = ?std::mem::discriminant(&pkt.data),
                    "dispatch_encode"
                );
                let _ = match pkt.data {
                    PacketPayload::LightUpdate(p) => conn.raw.append(&p),
                    PacketPayload::BlockUpdate(p) => conn.raw.append(&p),
                    PacketPayload::ChunkUnload(p) => conn.raw.append(&p),
                    PacketPayload::EntityPosSync(p) => conn.raw.append(&p),
                    PacketPayload::BlockDestruction(p) => conn.raw.append(&p),
                    PacketPayload::GameEvent(p) => conn.raw.append(&p),
                    PacketPayload::PlayerEnteredView(p) => conn.raw.append(&p),
                    PacketPayload::SetEntityData(p) => conn.raw.append(&p),
                    PacketPayload::SetEquipment(p) => conn.raw.append(&p),
                    PacketPayload::UpdateAttributes(p) => conn.raw.append(&p),
                    PacketPayload::SetPassengers(p) => conn.raw.append(&p),
                    PacketPayload::ChunkLoad {
                        column,
                        chunk_bytes,
                        heightmaps,
                        light_data,
                        block_entities,
                    } => {
                        let chunk_data = ChunkData {
                            heightmaps: heightmaps
                                .iter()
                                .map(|(kind, data)| (*kind, Cow::Borrowed(data.as_slice())))
                                .collect(),
                            data: chunk_bytes.as_slice(),
                            block_entities: Cow::Owned(block_entities),
                            ..Default::default()
                        };
                        conn.raw.append(&ClientboundLevelChunkWithLight {
                            pos: column,
                            chunk_data,
                            light_data,
                        })
                    }
                    PacketPayload::ChunkBatchStart(p) => conn.raw.append(&p),
                    PacketPayload::ChunkBatchFinished(p) => conn.raw.append(&p),
                    PacketPayload::PlayerLeftView(p) => conn.raw.append(&p),
                    PacketPayload::PlayerLogin(p) => conn.raw.append(&p),
                    PacketPayload::OpLevelEntityEvent(p) => conn.raw.append(&p),
                    PacketPayload::SetChunkCacheCenter(p) => conn.raw.append(&p),
                    PacketPayload::SetChunkCacheRadius(p) => conn.raw.append(&p),
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
                        conn.raw.append(&ClientboundPlayerInfoUpdate {
                            actions: PlayerListActions::new()
                                .with_add_player(true)
                                .with_update_game_mode(true)
                                .with_update_listed(true),
                            entries: Cow::Borrowed(&wire_entries),
                        })
                    }
                    PacketPayload::PlayerPosition(p) => conn.raw.append(&p),
                    PacketPayload::SystemChat(p) => conn.raw.append(&p),
                    PacketPayload::ContainerSetContent(p) => conn.raw.append(&p),
                    PacketPayload::ContainerSetSlot(p) => conn.raw.append(&p),
                    PacketPayload::SetCursorItem(p) => conn.raw.append(&p),
                    PacketPayload::SetHeldSlot(p) => conn.raw.append(&p),
                    PacketPayload::OpenScreen(p) => conn.raw.append(&p),
                    PacketPayload::ContainerClose(p) => conn.raw.append(&p),
                    PacketPayload::TakeItemEntity(p) => conn.raw.append(&p),
                    PacketPayload::Test(_) => {
                        // Test-only payload; no wire packet. Counted-drop so
                        // test assertions on encode_unhandled_total work.
                        telemetry.encode_unhandled_total += 1;
                        Ok(())
                    }
                };
            }
        }

        // --- (4) Coalesce + send ---
        // A tick's packets are one blob, and several ticks can fall in one frame, so the blob
        // is handed over in pieces the socket's queue can hold rather than counted against the
        // player: what it carries is what the server chose to send.
        let mut blob = conn.raw.take_encoded();
        while !blob.is_empty() {
            let piece = blob.split_to(blob.len().min(MAX_QUEUED_BYTES_PER_SOCKET));
            conn.raw.try_send_blob(piece);
        }
    }
}

/// Routes serverbound packets written to the bus into the dim channel seam.
///
/// `bridge_inbound` handles packets read from sockets; this system handles the
/// ones written by upstream callers. A full serverbound channel disconnects the
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
        if let Place::Joining(dim) | Place::Transferring { to: dim, .. } = placement.place() {
            placement.set(Place::InDim(dim));
        }
    }
}

/// Hands an attached session's held packets to its dimension.
pub fn forward_pending_inbound(
    mut sessions: Query<(&SessionPlacement, &mut PendingInbound)>,
    dim_channels: Res<DimChannelsResource>,
) {
    for (placement, mut pending) in &mut sessions {
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
            let _ = chan.serverbound_sender.try_send(ToDim::Serverbound(packet));
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
) {
    use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundDisconnect;

    for (entity, mut conn, mut bucket, anchor_ref, state) in conns.iter_mut() {
        if *state != ConnectionState::Game {
            continue;
        }
        bucket.refill();

        loop {
            match conn.raw.try_recv() {
                Ok(Some(pkt)) => {
                    if !bucket.consume_or_flag() {
                        conn.raw
                            .append(&ClientboundDisconnect {
                                reason: mcrs_minecraft_protocol::Text::from(
                                    "Connection flood detected",
                                ),
                            })
                            .ok();
                        let blob = conn.raw.take_encoded();
                        conn.raw.try_send_blob(blob);
                        commands.entity(entity).remove::<ServerSideConnection>();
                        telemetry.kick_flood_total += 1;
                        break;
                    }

                    commands.trigger(ReceivedPacketEvent {
                        entity,
                        id: pkt.id,
                        data: pkt.payload.clone(),
                        timestamp: pkt.timestamp,
                    });

                    if let Some(&HostAnchorRef(anchor)) = anchor_ref
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
                            },
                        )
                    {
                        commands.entity(entity).remove::<ServerSideConnection>();
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

    use crate::world::channel_types::{DimChannelsResource, FromDim, ToDim};
    use crate::world::session::SessionBundle;

    use bytes::Bytes;
    use mcrs_minecraft_level::session::PlayerSession;
    use mcrs_minecraft_level::world::channels::{
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
