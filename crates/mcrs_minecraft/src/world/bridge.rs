use std::sync::atomic::Ordering;

use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use bevy_ecs::message::{MessageReader, MessageWriter, Messages};
use bevy_ecs::prelude::Commands;
use bevy_ecs::query::{With, Without};
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::{Query, Res, ResMut};

/// FixedPostUpdate ordering for the three bridge stages.
///
/// `Outbound` fills per-connection `OutboundQueue` from the message bus.
/// `Dispatch` encodes + coalesces + sends each queue to the socket.
/// `Inbound` reads serverbound packets from sockets and routes them to
/// `PendingInboundPartition` or `inbound_pending`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub enum BridgeSet {
    Outbound,
    Dispatch,
    Inbound,
}
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{EngineConnection, InGameConnectionState, ServerSideConnection};
use mcrs_protocol::chunk::ChunkData;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundAddEntity, ClientboundBlockDestruction, ClientboundBlockUpdate,
    ClientboundChunkCacheRadius, ClientboundDisconnect, ClientboundEntityEvent,
    ClientboundEntityPositionSync, ClientboundForgetLevelChunk, ClientboundGameEvent,
    ClientboundLevelChunkWithLight, ClientboundLightUpdate, ClientboundLogin,
    ClientboundPlayerInfoUpdate, ClientboundPlayerPosition, ClientboundRemoveEntities,
    ClientboundSetChunkCacheCenter, ClientboundSystemChatPacket,
};
use mcrs_protocol::entity::player::PlayerSpawnInfo;
use mcrs_protocol::profile::{PlayerListActions, PlayerListEntry};
use mcrs_protocol::{ByteAngle, GameEventKind, Ident, Look, PositionFlag, Text, VarInt};
use tracing::{debug, trace, warn};

use crate::world::bridge_queue::{
    InboundRateBucket, OutboundQueue, DEPTH_DRAIN_TARGET, DEPTH_LIMIT, HIGH_OVERFLOW_LIMIT,
    KICK_AFTER_OVERFLOW_TICKS,
};
use mcrs_engine::session::{PlayerSession, SessionRegistry};
use crate::world::bus::{PacketPayload, PacketTarget};
use crate::world::channel_types::{
    send_control_or_teardown, DimChannelsResource, FromDim, ToDim,
};
use crate::world::player_index::{HostAnchorRef, PendingInboundBuffer};
use crate::world::sub_app_builder::{DimLabel, DimSubAppHandle};
use mcrs_engine::world::sub_app::DimDespawnQueue;

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
/// (`mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL`) rather than a
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
/// `PacketTarget` against `PlayerIndex`, and push packets onto the addressed
/// per-connection `OutboundQueue`.
///
/// Uses `reader.read()` (cursor semantics) so this is the single owning reader
/// of `OutboundPlayerPacket`. A second reader on the same type would produce an
/// independent cursor that re-reads from tick start — only one system may own
/// the reader.
///
/// A target that resolves to an entity with no `OutboundQueue` increments
/// `BRIDGE_OUTBOUND_NO_QUEUE_TOTAL` and is never silently dropped. This counter
/// makes any residual spawn→attach race observable without adding per-queue
/// atomics (no atomics for queue depth per CONVENTIONS §Concurrency).
pub fn bridge_outbound(
    mut reader: MessageReader<crate::world::bus::OutboundPlayerPacket>,
    session_registry: Res<SessionRegistry>,
    mut queues: Query<&mut OutboundQueue>,
) {
    for msg in reader.read() {
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_CONSUMED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        match &msg.target {
            PacketTarget::SinglePlayer(_) => {
                // Session + epoch stamped by the extract closure at the dim boundary.
                // PlayerSession(0) is never in the registry, so unstamped
                // packets are dropped here without an explicit check.
                let Some(entry) = session_registry.get(&msg.session) else {
                    continue;
                };
                if msg.epoch != entry.epoch {
                    continue;
                }
                let target_socket = entry.connection_entity;
                match queues.get_mut(target_socket) {
                    Ok(mut q) => q.push(msg.clone()),
                    Err(_) => {
                        mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            PacketTarget::AllInDim(dim_entity) => {
                // Broadcasts are generated for whoever is in the dim *now*, so
                // they are never stale: the per-session epoch stale-drop applies
                // only to SinglePlayer packets that may be in flight across a
                // transfer. Dimension isolation is already provided by
                // iter_in_dim. Epoch-filtering here would wrongly drop every
                // recipient whose epoch has advanced past a broadcast's
                // unstamped epoch.
                let dim = *dim_entity;
                let recipients: Vec<Entity> = session_registry
                    .iter_in_dim(dim)
                    .map(|(_, entry)| entry.connection_entity)
                    .collect();
                for socket in recipients {
                    match queues.get_mut(socket) {
                        Ok(mut q) => q.push(msg.clone()),
                        Err(_) => {
                            mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
            PacketTarget::AllPlayers => {
                // Not epoch-filtered — see AllInDim above. A fresh global
                // broadcast must reach every current session regardless of how
                // many dim transfers each has made.
                let recipients: Vec<Entity> = session_registry
                    .iter()
                    .map(|(_, entry)| entry.connection_entity)
                    .collect();
                for socket in recipients {
                    match queues.get_mut(socket) {
                        Ok(mut q) => q.push(msg.clone()),
                        Err(_) => {
                            mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
            PacketTarget::PlayerSet(set) => {
                // Not epoch-filtered — see AllInDim above. The recipient set is
                // the current observer set computed this tick; each member must
                // receive it at whatever epoch they currently hold.
                let recipients: Vec<Entity> = set
                    .iter()
                    .filter_map(|e| {
                        session_registry
                            .get_by_anchor(e)
                            .map(|(_, entry)| entry.connection_entity)
                    })
                    .collect();
                for socket in recipients {
                    match queues.get_mut(socket) {
                        Ok(mut q) => q.push(msg.clone()),
                        Err(_) => {
                            mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    }
}

/// Encode queued outbound packets for every active connection, enforce the
/// drop-oldest policy, kick connections that overflow Critical/High backlogs,
/// and coalesce all encoded bytes into a single `try_send_blob` per socket per
/// tick.
///
/// Execution order: runs in `BridgeSet::Dispatch` (FixedPostUpdate), after
/// `bridge_outbound` filled queues and before `bridge_inbound` reads.
///
/// SEQUENTIAL `iter_mut()` — do NOT use `par_iter_mut`. Kicking a connection
/// issues `commands.entity(e).remove::<ServerSideConnection>()`, which
/// requires exclusive Commands access not safe across parallel workers.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "network::dispatch_encode", skip_all)
)]
pub fn dispatch_encode(
    mut players: Query<(Entity, &mut OutboundQueue, &mut ServerSideConnection)>,
    mut commands: Commands,
) {
    use mcrs_network::metrics::{
        BRIDGE_DROP_LOW_TOTAL, BRIDGE_DROP_NORMAL_TOTAL, BRIDGE_ENCODE_UNHANDLED_TOTAL,
        BRIDGE_KICK_OVERFLOW_TOTAL, BRIDGE_QUEUE_DEPTH_CRITICAL, BRIDGE_QUEUE_DEPTH_HIGH,
        BRIDGE_QUEUE_DEPTH_LOW, BRIDGE_QUEUE_DEPTH_NORMAL,
    };
    use mcrs_network::MAX_QUEUED_BYTES_PER_SOCKET;

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
            commands.entity(entity).remove::<ServerSideConnection>();
            BRIDGE_KICK_OVERFLOW_TOTAL.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        // --- (1b) Critical/High overflow kick check ---
        if queue.critical_high_len() > HIGH_OVERFLOW_LIMIT {
            queue.overflow_ticks = queue.overflow_ticks.saturating_add(1);
        } else {
            queue.overflow_ticks = 0;
        }

        if queue.overflow_ticks >= KICK_AFTER_OVERFLOW_TICKS {
            conn.raw
                .append(&ClientboundDisconnect {
                    reason: Text::from("Server queue overflow"),
                })
                .ok();
            let blob = conn.raw.take_encoded();
            conn.raw.try_send_blob(blob);
            commands.entity(entity).remove::<ServerSideConnection>();
            BRIDGE_KICK_OVERFLOW_TOTAL.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        // --- (2) Drop policy: shed Normal first, then Low ---
        // Only activate if total exceeds DEPTH_LIMIT; then drain down to
        // DEPTH_DRAIN_TARGET so the queue stays below threshold for a few
        // ticks before refilling.
        if queue.total_len() > DEPTH_LIMIT {
            while queue.total_len() > DEPTH_DRAIN_TARGET {
                if queue.normal.pop_front().is_some() {
                    BRIDGE_DROP_NORMAL_TOTAL.fetch_add(1, Ordering::Relaxed);
                } else if queue.low.pop_front().is_some() {
                    BRIDGE_DROP_LOW_TOTAL.fetch_add(1, Ordering::Relaxed);
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
                match pkt.data {
                    PacketPayload::LightUpdate { column, light_data } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            col_x = column.x,
                            col_z = column.z,
                            "dispatch_encode: LightUpdate"
                        );
                        conn.raw
                            .append(&ClientboundLightUpdate {
                                x: VarInt(column.x),
                                z: VarInt(column.z),
                                light_data,
                            })
                            .ok();
                    }
                    PacketPayload::BlockUpdate {
                        position,
                        new_state,
                    } => {
                        conn.raw
                            .append(&ClientboundBlockUpdate {
                                block_pos: position,
                                block_state_id: new_state,
                            })
                            .ok();
                    }
                    PacketPayload::ChunkUnload { column } => {
                        conn.raw
                            .append(&ClientboundForgetLevelChunk {
                                x: column.x,
                                z: column.z,
                            })
                            .ok();
                    }
                    PacketPayload::EntityPosSync {
                        entity_id,
                        position,
                        velocity,
                        look,
                        on_ground,
                    } => {
                        trace!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            entity_id,
                            "dispatch_encode: EntityPosSync"
                        );
                        conn.raw
                            .append(&ClientboundEntityPositionSync {
                                entity_id: VarInt(entity_id),
                                position,
                                velocity,
                                look,
                                on_ground,
                            })
                            .ok();
                    }
                    PacketPayload::BlockDestruction {
                        entity_id,
                        pos,
                        progress,
                    } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            entity_id,
                            ?pos,
                            progress,
                            "dispatch_encode: BlockDestruction"
                        );
                        conn.raw
                            .append(&ClientboundBlockDestruction {
                                id: VarInt(entity_id),
                                pos,
                                progress,
                            })
                            .ok();
                    }
                    PacketPayload::GameEvent { game_event } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            "dispatch_encode: GameEvent"
                        );
                        conn.raw
                            .append(&ClientboundGameEvent { game_event })
                            .ok();
                    }
                    PacketPayload::PlayerEnteredView {
                        entity_id,
                        uuid,
                        kind,
                        position,
                        yaw,
                        pitch,
                    } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            entity_id,
                            "dispatch_encode: PlayerEnteredView"
                        );
                        conn.raw
                            .append(&ClientboundAddEntity {
                                id: VarInt(entity_id),
                                uuid,
                                kind: VarInt(kind),
                                pos: position,
                                velocity: VarInt(0),
                                yaw: ByteAngle::from_degrees(yaw),
                                pitch: ByteAngle::from_degrees(pitch),
                                head_yaw: ByteAngle::from_degrees(yaw),
                                data: VarInt(0),
                            })
                            .ok();
                    }
                    PacketPayload::ChunkLoad {
                        column,
                        chunk_bytes,
                        light_data,
                    } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            col_x = column.x,
                            col_z = column.z,
                            bytes = chunk_bytes.len(),
                            "dispatch_encode: ChunkLoad"
                        );
                        let chunk_data = ChunkData {
                            data: chunk_bytes.as_slice(),
                            ..Default::default()
                        };
                        conn.raw
                            .append(&ClientboundLevelChunkWithLight {
                                pos: column,
                                chunk_data,
                                light_data,
                            })
                            .ok();
                    }
                    PacketPayload::PlayerLeftView { entity_ids } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            count = entity_ids.len(),
                            "dispatch_encode: PlayerLeftView"
                        );
                        conn.raw
                            .append(&ClientboundRemoveEntities {
                                entity_ids: entity_ids.iter().map(|id| VarInt(*id)).collect(),
                            })
                            .ok();
                    }
                    PacketPayload::PlayerLogin {
                        player_id,
                        hardcore,
                        game_mode,
                        dimension,
                        dimension_type_id,
                        dimensions,
                        max_players,
                        chunk_radius,
                        simulation_distance,
                        reduced_debug_info,
                        show_death_screen,
                        do_limited_crafting,
                        enforces_secure_chat,
                    } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            player_id,
                            "dispatch_encode: PlayerLogin (releases client from Joining world)"
                        );
                        let dim_idents: Vec<Ident<std::borrow::Cow<str>>> = dimensions
                            .iter()
                            .filter_map(|s| {
                                Ident::<std::borrow::Cow<str>>::new(s.as_str()).ok()
                            })
                            .collect();
                        conn.raw
                            .append(&ClientboundLogin {
                                player_id,
                                hardcore,
                                dimensions: dim_idents,
                                max_players: VarInt(max_players),
                                chunk_radius: VarInt(chunk_radius),
                                simulation_distance: VarInt(simulation_distance),
                                reduced_debug_info,
                                show_death_screen,
                                do_limited_crafting,
                                player_spawn_info: PlayerSpawnInfo {
                                    dimension_type_id: VarInt(dimension_type_id),
                                    dimension: Ident::<std::borrow::Cow<str>>::new(
                                        dimension.as_str(),
                                    )
                                    .expect("dimension id is a valid resource location"),
                                    game_mode,
                                    ..Default::default()
                                },
                                enforces_secure_chat,
                            })
                            .ok();
                    }
                    PacketPayload::LevelChunksLoadStart => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            "dispatch_encode: LevelChunksLoadStart"
                        );
                        conn.raw
                            .append(&ClientboundGameEvent {
                                game_event: GameEventKind::LevelChunksLoadStart,
                            })
                            .ok();
                    }
                    PacketPayload::PlayerLoginEntityEvent {
                        entity_id,
                        entity_status,
                    } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            entity_id,
                            entity_status,
                            "dispatch_encode: PlayerLoginEntityEvent"
                        );
                        conn.raw
                            .append(&ClientboundEntityEvent {
                                entity_id,
                                entity_status,
                            })
                            .ok();
                    }
                    PacketPayload::SetChunkCacheCenter { x, z } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            x,
                            z,
                            "dispatch_encode: SetChunkCacheCenter"
                        );
                        conn.raw
                            .append(&ClientboundSetChunkCacheCenter {
                                x: VarInt(x),
                                z: VarInt(z),
                            })
                            .ok();
                    }
                    PacketPayload::SetChunkCacheRadius { radius } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            radius,
                            "dispatch_encode: SetChunkCacheRadius"
                        );
                        conn.raw
                            .append(&ClientboundChunkCacheRadius {
                                radius: VarInt(radius),
                            })
                            .ok();
                    }
                    PacketPayload::PlayerInfoUpdate { entries } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            count = entries.len(),
                            "dispatch_encode: PlayerInfoUpdate"
                        );
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
                        conn.raw
                            .append(&ClientboundPlayerInfoUpdate {
                                actions: PlayerListActions::new()
                                    .with_add_player(true)
                                    .with_update_game_mode(true)
                                    .with_update_listed(true),
                                entries: std::borrow::Cow::Borrowed(&wire_entries),
                            })
                            .ok();
                    }
                    PacketPayload::PlayerPosition {
                        teleport_id,
                        position,
                    } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            ?position,
                            teleport_id,
                            "dispatch_encode: PlayerPosition (teleport-sync)"
                        );
                        conn.raw
                            .append(&ClientboundPlayerPosition {
                                teleport_id: VarInt(teleport_id),
                                position,
                                velocity: DVec3::ZERO,
                                look: Look::default(),
                                flags: Vec::<PositionFlag>::new(),
                            })
                            .ok();
                    }
                    PacketPayload::SystemChat { content, overlay } => {
                        debug!(
                            target: "mcrs_minecraft::bridge",
                            conn = ?entity,
                            "dispatch_encode: SystemChat"
                        );
                        conn.raw
                            .append(&ClientboundSystemChatPacket { content, overlay })
                            .ok();
                    }
                    PacketPayload::Test(_) => {
                        // Test-only payload; no wire packet. Counted-drop so
                        // test assertions on BRIDGE_ENCODE_UNHANDLED_TOTAL work.
                        BRIDGE_ENCODE_UNHANDLED_TOTAL.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        // --- (4) Coalesce + send ---
        let blob = conn.raw.take_encoded();
        if blob.len() > MAX_QUEUED_BYTES_PER_SOCKET {
            // Byte-cap backstop: oversized blob is never sent; kick the connection.
            warn!(
                entity = ?entity,
                blob_len = blob.len(),
                max = MAX_QUEUED_BYTES_PER_SOCKET,
                "dispatch_encode: blob exceeds MAX_QUEUED_BYTES_PER_SOCKET; closing connection"
            );
            commands.entity(entity).remove::<ServerSideConnection>();
            continue;
        }
        if !blob.is_empty() && !conn.raw.try_send_blob(blob) {
            // Channel full = backpressure; feeds kick path next tick.
            queue.overflow_ticks = queue.overflow_ticks.saturating_add(1);
        }

        // --- (5) Update depth gauges (monotone totals, consistent with metrics.rs) ---
        BRIDGE_QUEUE_DEPTH_CRITICAL.fetch_add(queue.critical.len() as u64, Ordering::Relaxed);
        BRIDGE_QUEUE_DEPTH_HIGH.fetch_add(queue.high.len() as u64, Ordering::Relaxed);
        BRIDGE_QUEUE_DEPTH_NORMAL.fetch_add(queue.normal.len() as u64, Ordering::Relaxed);
        BRIDGE_QUEUE_DEPTH_LOW.fetch_add(queue.low.len() as u64, Ordering::Relaxed);
    }
}

/// Routes serverbound packets from the network into the dim channel seam.
///
/// Drains `Messages<OutboundPlayerPacket>` is the outbound side; this system
/// handles the inbound side: for each `InboundPlayerPacket` written by upstream
/// callers, resolve the player's dim and `try_send` it as `ToDim::Serverbound`
/// into the dim's bounded serverbound channel. On `TrySendError::Full`,
/// disconnect the offending session (D-08b). Pre-attach players whose dim is
/// still `PLACEHOLDER` are held in `PendingInboundBuffer` until
/// `bridge_player_attach` drains them.
pub fn bridge_inbound_to_channel(
    mut msgs: ResMut<Messages<crate::world::bus::InboundPlayerPacket>>,
    session_registry: Res<SessionRegistry>,
    dim_channels: Res<DimChannelsResource>,
    mut inbound_buffer: ResMut<PendingInboundBuffer>,
    mut commands: Commands,
) {
    use flume::TrySendError;
    for msg in msgs.drain() {
        let Some((_, entry)) = session_registry.get_by_anchor(&msg.player) else {
            continue;
        };
        if entry.in_dim_entity.is_some() && entry.dim != Entity::PLACEHOLDER {
            let Some(chan) = dim_channels.get(entry.dim) else {
                continue;
            };
            match chan.serverbound_sender.try_send(ToDim::Serverbound {
                player: msg.player,
                id: msg.id,
                data: msg.data,
                timestamp: msg.timestamp,
            }) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    if let Some((_, sess_entry)) = session_registry.get_by_anchor(&msg.player) {
                        commands
                            .entity(sess_entry.connection_entity)
                            .remove::<mcrs_network::ServerSideConnection>();
                    }
                }
                Err(TrySendError::Disconnected(_)) => {}
            }
        } else {
            inbound_buffer.buffers.entry(msg.player).or_default().push(msg);
        }
    }
}

pub fn bridge_player_attach(
    mut attach_msgs: ResMut<Messages<crate::world::bus::OutboundPlayerAttached>>,
    mut session_registry: ResMut<SessionRegistry>,
    mut inbound_buffer: ResMut<PendingInboundBuffer>,
    dim_channels: Res<DimChannelsResource>,
) {
    for msg in attach_msgs.drain() {
        let Some((_, entry)) = session_registry.get_by_anchor_mut(&msg.host_anchor) else {
            continue;
        };
        entry.in_dim_entity = Some(msg.new_in_dim_entity);
        entry.previous_dim = None;
        let current_dim = entry.dim;

        if let Some(buffered) = inbound_buffer.buffers.remove(&msg.host_anchor) {
            if let Some(chan) = dim_channels.get(current_dim) {
                for packet in buffered {
                    let _ = chan.serverbound_sender.try_send(ToDim::Serverbound {
                        player: packet.player,
                        id: packet.id,
                        data: packet.data,
                        timestamp: packet.timestamp,
                    });
                }
            }
        }
    }
}

pub fn bridge_inbound(
    mut conns: Query<
        (
            Entity,
            &mut ServerSideConnection,
            &mut InboundRateBucket,
            Option<&HostAnchorRef>,
        ),
        With<InGameConnectionState>,
    >,
    mut commands: Commands,
    session_registry: Res<SessionRegistry>,
    dim_channels: Res<DimChannelsResource>,
    mut inbound_buffer: ResMut<PendingInboundBuffer>,
) {
    use flume::TrySendError;
    use mcrs_network::metrics::BRIDGE_KICK_FLOOD_TOTAL;
    use mcrs_protocol::packets::game::clientbound::ClientboundDisconnect;

    for (entity, mut conn, mut bucket, anchor_ref) in conns.iter_mut() {
        bucket.refill();

        loop {
            match conn.raw.try_recv() {
                Ok(Some(pkt)) => {
                    if !bucket.consume_or_flag() {
                        conn.raw
                            .append(&ClientboundDisconnect {
                                reason: mcrs_protocol::Text::from("Connection flood detected"),
                            })
                            .ok();
                        let blob = conn.raw.take_encoded();
                        conn.raw.try_send_blob(blob);
                        commands.entity(entity).remove::<ServerSideConnection>();
                        BRIDGE_KICK_FLOOD_TOTAL.fetch_add(1, Ordering::Relaxed);
                        break;
                    }

                    commands.trigger(ReceivedPacketEvent {
                        entity,
                        id: pkt.id,
                        data: pkt.payload.clone(),
                        timestamp: pkt.timestamp,
                    });

                    if let Some(anchor) = anchor_ref {
                        if let Some((_, entry)) = session_registry.get_by_anchor(&anchor.0) {
                            if entry.dim != Entity::PLACEHOLDER {
                                if entry.in_dim_entity.is_some() {
                                    if let Some(chan) = dim_channels.get(entry.dim) {
                                        match chan.serverbound_sender.try_send(ToDim::Serverbound {
                                            player: anchor.0,
                                            id: pkt.id,
                                            data: pkt.payload,
                                            timestamp: pkt.timestamp,
                                        }) {
                                            Ok(()) => {}
                                            Err(TrySendError::Full(_)) => {
                                                commands
                                                    .entity(entity)
                                                    .remove::<ServerSideConnection>();
                                            }
                                            Err(TrySendError::Disconnected(_)) => {}
                                        }
                                    }
                                } else {
                                    inbound_buffer
                                        .buffers
                                        .entry(anchor.0)
                                        .or_default()
                                        .push(crate::world::bus::InboundPlayerPacket {
                                            player: anchor.0,
                                            id: pkt.id,
                                            data: pkt.payload,
                                            timestamp: pkt.timestamp,
                                        });
                                }
                            }
                        }
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
    use smallvec::SmallVec;

    use bytes::Bytes;
    use mcrs_engine::session::{PlayerSession, SessionEntry, SessionRegistry};
    use mcrs_engine::world::channels::{DimSender, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY};
    use crate::world::bus::{InboundPlayerPacket, PlayerTransferSnapshot};
    use crate::world::channel_types::{DimChannelsResource, FromDim, ToDim};
    use crate::world::player_index::PendingInboundBuffer;
    use bevy_math::{DVec3, Vec2};
    use mcrs_protocol::uuid::Uuid;

    fn make_session_entry(
        connection_entity: Entity,
        host_anchor: Entity,
        dim: Entity,
        in_dim_entity: Option<Entity>,
    ) -> SessionEntry {
        SessionEntry {
            connection_entity,
            host_anchor,
            dim,
            previous_dim: None,
            in_dim_entity,
            epoch: 0,
        }
    }

    fn make_dim_channels(world: &mut World, dim: Entity) -> (flume::Receiver<ToDim>, flume::Receiver<ToDim>, flume::Sender<FromDim>) {
        let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
        let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
        let (from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
        world
            .resource_mut::<DimChannelsResource>()
            .insert(dim, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
        (srv_rx, ctl_rx, from_tx)
    }

    fn run_attach(world: &mut World) {
        let mut sys = IntoSystem::into_system(bridge_player_attach);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    #[test]
    fn bridge_player_attach_sets_in_dim_entity_and_sends_buffered_packets() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::world::bus::OutboundPlayerAttached>>();
        world.init_resource::<SessionRegistry>();
        world.init_resource::<PendingInboundBuffer>();
        world.init_resource::<DimChannelsResource>();

        let host_anchor = Entity::from_raw_u32(42).expect("nonzero");
        let connection_entity = Entity::from_raw_u32(1).expect("nonzero");
        let dest_dim = Entity::from_raw_u32(2).expect("nonzero");
        let new_in_dim = Entity::from_raw_u32(200).expect("nonzero");
        let session = PlayerSession(3);

        let (dest_srv_rx, _dest_ctl_rx, _dest_from_tx) = make_dim_channels(&mut world, dest_dim);

        world.resource_mut::<SessionRegistry>().insert(
            session,
            SessionEntry {
                connection_entity,
                host_anchor,
                dim: dest_dim,
                previous_dim: None,
                in_dim_entity: None,
                epoch: 0,
            },
        );

        let mut buffered: SmallVec<[InboundPlayerPacket; 4]> = SmallVec::new();
        for seq in 0..3u32 {
            buffered.push(InboundPlayerPacket {
                player: host_anchor,
                id: seq as i32,
                data: Bytes::new(),
                timestamp: std::time::Instant::now(),
            });
        }
        world.resource_mut::<PendingInboundBuffer>().buffers.insert(host_anchor, buffered);

        world
            .resource_mut::<Messages<crate::world::bus::OutboundPlayerAttached>>()
            .write(crate::world::bus::OutboundPlayerAttached {
                host_anchor,
                new_in_dim_entity: new_in_dim,
            });

        run_attach(&mut world);

        let registry = world.resource::<SessionRegistry>();
        let (_, entry) = registry.get_by_anchor(&host_anchor).expect("entry present");
        assert_eq!(entry.in_dim_entity, Some(new_in_dim));

        let buffer = world.resource::<PendingInboundBuffer>();
        assert!(buffer.buffers.get(&host_anchor).map_or(true, |v| v.is_empty()));

        let drained: Vec<_> = dest_srv_rx.try_iter().collect();
        assert_eq!(drained.len(), 3, "3 buffered packets sent to serverbound channel");
    }

    #[test]
    fn bridge_player_attach_idempotent_on_unknown_host_anchor() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::world::bus::OutboundPlayerAttached>>();
        world.init_resource::<SessionRegistry>();
        world.init_resource::<PendingInboundBuffer>();
        world.init_resource::<DimChannelsResource>();

        let unknown = Entity::from_raw_u32(999).expect("nonzero");
        let new_in_dim = Entity::from_raw_u32(1).expect("nonzero");

        world
            .resource_mut::<Messages<crate::world::bus::OutboundPlayerAttached>>()
            .write(crate::world::bus::OutboundPlayerAttached {
                host_anchor: unknown,
                new_in_dim_entity: new_in_dim,
            });

        run_attach(&mut world);
        // No panic — idempotent on unknown anchor
    }
}
