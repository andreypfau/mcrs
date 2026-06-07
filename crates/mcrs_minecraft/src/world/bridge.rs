use std::sync::atomic::Ordering;

use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use bevy_ecs::message::{MessageReader, Messages};
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
    ClientboundAddEntity, ClientboundBlockUpdate, ClientboundChunkCacheRadius,
    ClientboundDisconnect, ClientboundEntityEvent, ClientboundEntityPositionSync,
    ClientboundForgetLevelChunk, ClientboundGameEvent, ClientboundLevelChunkWithLight,
    ClientboundLightUpdate, ClientboundLogin, ClientboundPlayerInfoUpdate,
    ClientboundPlayerPosition, ClientboundRemoveEntities, ClientboundSetChunkCacheCenter,
};
use mcrs_protocol::entity::player::PlayerSpawnInfo;
use mcrs_protocol::profile::{PlayerListActions, PlayerListEntry};
use mcrs_protocol::{ByteAngle, GameEventKind, Ident, Look, PositionFlag, Text, VarInt};
use rustc_hash::FxHashSet;
use tracing::{debug, trace, warn};

use crate::world::bridge_queue::{
    InboundRateBucket, OutboundQueue, DEPTH_DRAIN_TARGET, DEPTH_LIMIT, HIGH_OVERFLOW_LIMIT,
    KICK_AFTER_OVERFLOW_TICKS,
};
use crate::world::bus::{
    InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerTransfer,
    PacketPayload, PacketTarget, PendingInboundLifecycle, PendingInboundPartition,
};
use crate::world::player_index::{HostAnchorRef, PlayerIndex};
use crate::world::sub_app_builder::DimSubAppHandle;

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
    player_index: Res<PlayerIndex>,
    mut queues: Query<&mut OutboundQueue>,
    _anchors: Query<&HostAnchorRef>,
) {
    for msg in reader.read() {
        mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_CONSUMED_TOTAL
            .fetch_add(1, Ordering::Relaxed);

        match &msg.target {
            PacketTarget::SinglePlayer(player_entity) => {
                let Some(loc) = player_index.get(player_entity) else {
                    continue;
                };
                let target_socket = loc.socket;
                match queues.get_mut(target_socket) {
                    Ok(mut q) => q.push(msg.clone()),
                    Err(_) => {
                        mcrs_network::metrics::BRIDGE_OUTBOUND_NO_QUEUE_TOTAL
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            PacketTarget::AllInDim(dim_entity) => {
                let dim = *dim_entity;
                let sockets: Vec<Entity> = player_index
                    .iter()
                    .filter_map(|(_, loc)| {
                        if loc.current_dim == dim {
                            Some(loc.socket)
                        } else {
                            None
                        }
                    })
                    .collect();
                for socket in sockets {
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
                let sockets: Vec<Entity> =
                    player_index.iter().map(|(_, loc)| loc.socket).collect();
                for socket in sockets {
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
                let sockets: Vec<Entity> = set
                    .iter()
                    .filter_map(|e| player_index.get(e).map(|loc| loc.socket))
                    .collect();
                for socket in sockets {
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

pub fn partition_main_inbound(
    mut msgs: ResMut<Messages<InboundPlayerPacket>>,
    mut partition: ResMut<PendingInboundPartition>,
    mut player_index: ResMut<PlayerIndex>,
) {
    for msg in msgs.drain() {
        let Some(location) = player_index.get_mut(&msg.player) else {
            continue;
        };
        // current_dim == PLACEHOLDER means the login path inserted the
        // PlayerIndex entry before spawn-point selection assigned a real
        // dim. Routing into partition.per_dim[PLACEHOLDER] would land in a
        // bucket that no sub-app extract drains, leaking the packet. Hold
        // it in inbound_pending until bridge_player_attach fires.
        if location.in_dim_entity.is_some() && location.current_dim != Entity::PLACEHOLDER {
            partition
                .per_dim
                .entry(location.current_dim)
                .or_default()
                .push(msg);
        } else {
            location.inbound_pending.push(msg);
        }
    }
}

pub fn bridge_player_transfer(
    mut transfer_msgs: ResMut<Messages<OutboundPlayerTransfer>>,
    mut player_index: ResMut<PlayerIndex>,
    mut lifecycle: ResMut<PendingInboundLifecycle>,
    live_dims: Query<Entity, With<DimSubAppHandle>>,
) {
    // Snapshot the set of live sub-app label entities once per system
    // run; an OutboundPlayerTransfer carrying a dest_dim that does not
    // match a live handle would leave the player's current_dim pointing
    // at a sub-app that no extract closure drains, and the spawn would
    // accumulate in lifecycle.per_dim[dest_dim] indefinitely.
    let valid_dims: FxHashSet<Entity> = live_dims.iter().collect();
    for msg in transfer_msgs.drain() {
        if !valid_dims.contains(&msg.dest_dim) {
            warn!(
                host_anchor = ?msg.host_anchor,
                dest_dim = ?msg.dest_dim,
                "OutboundPlayerTransfer targets a dim entity not registered as a live sub-app; dropping"
            );
            continue;
        }
        let Some(location) = player_index.get_mut(&msg.host_anchor) else {
            continue;
        };
        let old_current_dim = location.current_dim;
        location.current_dim = msg.dest_dim;
        location.previous_dim = Some(old_current_dim);
        location.in_dim_entity = None;
        let spawn = InboundPlayerSpawn {
            host_anchor: msg.host_anchor,
            snapshot: msg.snapshot.clone(),
        };
        lifecycle
            .per_dim
            .entry(msg.dest_dim)
            .or_default()
            .spawns
            .push(spawn);
    }
}

pub fn bridge_player_attach(
    mut attach_msgs: ResMut<Messages<OutboundPlayerAttached>>,
    mut player_index: ResMut<PlayerIndex>,
    mut partition: ResMut<PendingInboundPartition>,
) {
    for msg in attach_msgs.drain() {
        let drained_and_dim = {
            let Some(location) = player_index.get_mut(&msg.host_anchor) else {
                continue;
            };
            location.in_dim_entity = Some(msg.new_in_dim_entity);
            location.previous_dim = None;
            let drained = std::mem::take(&mut location.inbound_pending);
            let current_dim = location.current_dim;
            (drained, current_dim)
        };
        let (drained, current_dim) = drained_and_dim;
        if !drained.is_empty() {
            let bucket = partition.per_dim.entry(current_dim).or_default();
            for packet in drained {
                bucket.push(packet);
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
    player_index: Res<PlayerIndex>,
    mut partition: ResMut<PendingInboundPartition>,
) {
    use mcrs_network::metrics::BRIDGE_KICK_FLOOD_TOTAL;
    use mcrs_protocol::packets::game::clientbound::ClientboundDisconnect;

    for (entity, mut conn, mut bucket, anchor_ref) in conns.iter_mut() {
        bucket.refill();

        loop {
            match conn.raw.try_recv() {
                Ok(Some(pkt)) => {
                    if !bucket.consume_or_flag() {
                        // Flood threshold exceeded — kick with reason.
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

                    // Host-world re-emit: drives host-registered observers
                    // (keepalive, accept-teleportation, login/config).
                    commands.trigger(ReceivedPacketEvent {
                        entity,
                        id: pkt.id,
                        data: pkt.payload.clone(),
                        timestamp: pkt.timestamp,
                    });

                    // Dim-world route: push the real packet into the player's
                    // current-dimension partition. The per-dim extract closure
                    // shuttles it into the sub-world's Messages<InboundPlayerPacket>,
                    // where dispatch_inbound_to_dim re-emits it as a
                    // ReceivedPacketEvent on the in-dim player entity so dim
                    // observers (movement, chat, digging) fire.
                    if let Some(anchor) = anchor_ref {
                        if let Some(location) = player_index.get(&anchor.0) {
                            if location.current_dim != Entity::PLACEHOLDER {
                                partition
                                    .per_dim
                                    .entry(location.current_dim)
                                    .or_default()
                                    .push(InboundPlayerPacket {
                                        player: anchor.0,
                                        id: pkt.id,
                                        data: pkt.payload,
                                        timestamp: pkt.timestamp,
                                    });
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    // Channel disconnected — writer died; remove the connection.
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
    use crate::world::player_index::PlayerLocation;

    fn make_location(
        current_dim: Entity,
        in_dim_entity: Option<Entity>,
    ) -> PlayerLocation {
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim,
            previous_dim: None,
            in_dim_entity,
            inbound_pending: SmallVec::new(),
        }
    }

    fn run_partition(world: &mut World) {
        let mut sys = IntoSystem::into_system(partition_main_inbound);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    fn write_inbound(world: &mut World, msg: InboundPlayerPacket) {
        world
            .resource_mut::<Messages<InboundPlayerPacket>>()
            .write(msg);
    }

    #[test]
    fn partition_routes_to_pending_when_in_dim_entity_is_some() {
        let mut world = World::new();
        world.init_resource::<Messages<InboundPlayerPacket>>();
        world.init_resource::<PendingInboundPartition>();
        world.init_resource::<PlayerIndex>();

        let player = Entity::from_raw_u32(100).expect("nonzero");
        let dim = Entity::from_raw_u32(7).expect("nonzero");
        let in_dim = Entity::from_raw_u32(42).expect("nonzero");

        world
            .resource_mut::<PlayerIndex>()
            .insert(player, make_location(dim, Some(in_dim)));
        write_inbound(
            &mut world,
            InboundPlayerPacket {
                player,
                id: 1,
                data: Bytes::new(),
                timestamp: std::time::Instant::now(),
            },
        );

        run_partition(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert_eq!(partition.per_dim.get(&dim).map(|v| v.len()), Some(1));

        let index = world.resource::<PlayerIndex>();
        let location = index.get(&player).expect("present");
        assert!(location.inbound_pending.is_empty());
    }

    #[test]
    fn partition_routes_to_inbound_pending_when_in_dim_entity_is_none() {
        let mut world = World::new();
        world.init_resource::<Messages<InboundPlayerPacket>>();
        world.init_resource::<PendingInboundPartition>();
        world.init_resource::<PlayerIndex>();

        let player = Entity::from_raw_u32(100).expect("nonzero");
        let dim = Entity::from_raw_u32(7).expect("nonzero");

        world
            .resource_mut::<PlayerIndex>()
            .insert(player, make_location(dim, None));
        write_inbound(
            &mut world,
            InboundPlayerPacket {
                player,
                id: 2,
                data: Bytes::new(),
                timestamp: std::time::Instant::now(),
            },
        );

        run_partition(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert!(partition.per_dim.is_empty());

        let index = world.resource::<PlayerIndex>();
        let location = index.get(&player).expect("present");
        assert_eq!(location.inbound_pending.len(), 1);
    }

    #[test]
    fn partition_drops_unknown_player_silently() {
        let mut world = World::new();
        world.init_resource::<Messages<InboundPlayerPacket>>();
        world.init_resource::<PendingInboundPartition>();
        world.init_resource::<PlayerIndex>();

        let unknown = Entity::from_raw_u32(999).expect("nonzero");
        write_inbound(
            &mut world,
            InboundPlayerPacket {
                player: unknown,
                id: 3,
                data: Bytes::new(),
                timestamp: std::time::Instant::now(),
            },
        );

        run_partition(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert!(partition.per_dim.is_empty());

        let msgs = world.resource::<Messages<InboundPlayerPacket>>();
        let mut reader = msgs.get_cursor();
        assert_eq!(reader.read(msgs).count(), 0);
    }

    use crate::world::bus::PlayerTransferSnapshot;
    use bevy_math::{DVec3, Vec2};
    use mcrs_protocol::uuid::Uuid;

    fn synthetic_snapshot() -> PlayerTransferSnapshot {
        PlayerTransferSnapshot {
            uuid: Uuid::nil(),
            username: "test".into(),
            position: DVec3::ZERO,
            rotation: Vec2::ZERO,
        }
    }

    fn run_transfer(world: &mut World) {
        let mut sys = IntoSystem::into_system(bridge_player_transfer);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    fn run_attach(world: &mut World) {
        let mut sys = IntoSystem::into_system(bridge_player_attach);
        sys.initialize(world);
        let _ = sys.run((), world);
        sys.apply_deferred(world);
    }

    #[test]
    fn bridge_player_transfer_updates_player_index_and_writes_spawn() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerTransfer>>();
        world.init_resource::<Messages<InboundPlayerSpawn>>();
        world.init_resource::<PlayerIndex>();
        world.init_resource::<PendingInboundLifecycle>();

        let host_anchor = world.spawn_empty().id();
        let src_dim = world.spawn(DimSubAppHandle).id();
        let dest_dim = world.spawn(DimSubAppHandle).id();
        let src_in_dim = world.spawn_empty().id();

        world.resource_mut::<PlayerIndex>().insert(
            host_anchor,
            PlayerLocation {
                socket: Entity::PLACEHOLDER,
                current_dim: src_dim,
                previous_dim: None,
                in_dim_entity: Some(src_in_dim),
                inbound_pending: SmallVec::new(),
            },
        );

        world
            .resource_mut::<Messages<OutboundPlayerTransfer>>()
            .write(OutboundPlayerTransfer {
                host_anchor,
                dest_dim,
                snapshot: synthetic_snapshot(),
            });

        run_transfer(&mut world);

        let index = world.resource::<PlayerIndex>();
        let loc = index.get(&host_anchor).expect("present");
        assert_eq!(loc.current_dim, dest_dim);
        assert!(loc.in_dim_entity.is_none());

        let lifecycle = world.resource::<PendingInboundLifecycle>();
        let bundle = lifecycle
            .per_dim
            .get(&dest_dim)
            .expect("dest dim bundle present");
        assert_eq!(bundle.spawns.len(), 1);
        assert_eq!(bundle.spawns[0].host_anchor, host_anchor);
    }

    #[test]
    fn bridge_player_transfer_drops_transfer_to_unregistered_dim() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerTransfer>>();
        world.init_resource::<Messages<InboundPlayerSpawn>>();
        world.init_resource::<PlayerIndex>();
        world.init_resource::<PendingInboundLifecycle>();

        let host_anchor = world.spawn_empty().id();
        let src_dim = world.spawn(DimSubAppHandle).id();
        let src_in_dim = world.spawn_empty().id();
        // dest_dim is allocated but NOT carrying DimSubAppHandle, so it
        // does not represent a live sub-app.
        let bogus_dim = world.spawn_empty().id();

        world.resource_mut::<PlayerIndex>().insert(
            host_anchor,
            PlayerLocation {
                socket: Entity::PLACEHOLDER,
                current_dim: src_dim,
                previous_dim: None,
                in_dim_entity: Some(src_in_dim),
                inbound_pending: SmallVec::new(),
            },
        );

        world
            .resource_mut::<Messages<OutboundPlayerTransfer>>()
            .write(OutboundPlayerTransfer {
                host_anchor,
                dest_dim: bogus_dim,
                snapshot: synthetic_snapshot(),
            });

        run_transfer(&mut world);

        // PlayerIndex remains at src_dim — the transfer was dropped.
        let index = world.resource::<PlayerIndex>();
        let loc = index.get(&host_anchor).expect("present");
        assert_eq!(loc.current_dim, src_dim);
        assert_eq!(loc.in_dim_entity, Some(src_in_dim));

        // Lifecycle bucket for the bogus dim must not have been created.
        let lifecycle = world.resource::<PendingInboundLifecycle>();
        assert!(!lifecycle.per_dim.contains_key(&bogus_dim));
    }

    #[test]
    fn bridge_player_attach_sets_in_dim_entity_and_drains_inbound_pending() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerAttached>>();
        world.init_resource::<PlayerIndex>();
        world.init_resource::<PendingInboundPartition>();

        let host_anchor = Entity::from_raw_u32(42).expect("nonzero");
        let dest_dim = Entity::from_raw_u32(2).expect("nonzero");
        let new_in_dim = Entity::from_raw_u32(200).expect("nonzero");

        let mut buffered: SmallVec<[InboundPlayerPacket; 4]> = SmallVec::new();
        for seq in 0..3u32 {
            buffered.push(InboundPlayerPacket {
                player: host_anchor,
                id: seq as i32,
                data: Bytes::new(),
                timestamp: std::time::Instant::now(),
            });
        }

        world.resource_mut::<PlayerIndex>().insert(
            host_anchor,
            PlayerLocation {
                socket: Entity::PLACEHOLDER,
                current_dim: dest_dim,
                previous_dim: None,
                in_dim_entity: None,
                inbound_pending: buffered,
            },
        );

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached {
                host_anchor,
                new_in_dim_entity: new_in_dim,
            });

        run_attach(&mut world);

        let index = world.resource::<PlayerIndex>();
        let loc = index.get(&host_anchor).expect("present");
        assert_eq!(loc.in_dim_entity, Some(new_in_dim));
        assert!(loc.inbound_pending.is_empty());

        let partition = world.resource::<PendingInboundPartition>();
        let dest_bucket = partition
            .per_dim
            .get(&dest_dim)
            .expect("dest dim bucket present");
        assert_eq!(dest_bucket.len(), 3);
    }

    #[test]
    fn bridge_player_attach_idempotent_on_unknown_host_anchor() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerAttached>>();
        world.init_resource::<PlayerIndex>();
        world.init_resource::<PendingInboundPartition>();

        let unknown = Entity::from_raw_u32(999).expect("nonzero");
        let new_in_dim = Entity::from_raw_u32(1).expect("nonzero");

        world
            .resource_mut::<Messages<OutboundPlayerAttached>>()
            .write(OutboundPlayerAttached {
                host_anchor: unknown,
                new_in_dim_entity: new_in_dim,
            });

        run_attach(&mut world);

        let partition = world.resource::<PendingInboundPartition>();
        assert!(partition.per_dim.is_empty());
    }
}
