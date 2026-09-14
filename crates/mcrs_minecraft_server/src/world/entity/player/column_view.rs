use crate::world::light_codec::{
    LightCodecParams, build_full_light_data, build_fullbright_light_data,
};
use bevy_app::{App, FixedUpdate, Plugin, PreUpdate};
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Added, Component, ContainsEntity, MessageReader, On, Query, With};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::Commands;
use mcrs_minecraft_block::palette::{AirCount, BiomePalette, ChunkBlocks};
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::chunk::ChunkDataBlockEntity;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChunkBatchReceived;
use mcrs_minecraft_protocol::{ColumnPos, Encode};
use mcrs_voxel_world::entity::player::chunk_view::{
    ChunkTrackingViewUpdateEvent, ChunkViewPlugin, ChunkViewSet, PlayerChunkLoadRequest,
    PlayerChunkObserver, PlayerChunkUnloadRequest,
};
use mcrs_voxel_world::entity::player::reposition::Reposition;
use mcrs_voxel_world::session::PlayerSession;
use mcrs_voxel_world::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_voxel_world::world::lifecycle::markers::ChunkLoaded;
use mcrs_voxel_world::world::lifecycle::ticket::ChunkSpawnSet;
use mcrs_voxel_world::world::lifecycle::ticket::{ChunkTicketsCommands, Ticket, TicketKind};
use mcrs_voxel_world::world::lifecycle::trace as column_trace;
use mcrs_voxel_world::world::lifecycle::trace::ColumnStage;
use mcrs_voxel_world::world::storage::block_entity::SectionBlockEntities;
use mcrs_voxel_world::world::storage::chunk::ChunkIndex;
use mcrs_voxel_world::world::storage::column::{ColumnIndex, ColumnPos as EngineColumnPos};

use crate::world::block_entity::{BlockEntity, packet_entry};
use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;
use crate::world::heightmap::{
    MotionHeightmap, NoLeavesHeightmap, SurfaceHeightmap, client_heightmaps,
};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{trace, warn};

pub struct ColumnViewPlugin;

/// Turns the view's requests into tickets, so the spawn that follows in the same tick sees
/// them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnViewSet;

impl Plugin for ColumnViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ChunkViewPlugin);

        // Initialize per-player column state.
        app.add_systems(PreUpdate, add_player_column_view);

        app.configure_sets(
            FixedUpdate,
            (ChunkViewSet, ColumnViewSet, ChunkSpawnSet).chain(),
        );
        app.add_systems(
            FixedUpdate,
            (
                unload_chunk_request,
                request_columns,
                project_ready_columns,
                send_column_queue,
            )
                .chain()
                .in_set(ColumnViewSet),
        );

        // React to ChunkTrackingView changes (xz-distance changes, movement).
        app.add_observer(on_view_update);
        app.add_observer(handle_batch_acknowledgement);
        app.add_observer(release_forced_tickets);

        // Progressively ticket columns closest to the player, then send loaded ones.
        // app.add_systems(
        //     FixedUpdate,
        //     (ticket_pending_columns, process_column_queues).chain(),
        // );
    }
}

/// What a player is owed, split by how far along it is. A column the view wants sits in
/// exactly one of the three sets, so the sets together are the view's want list and no fourth
/// copy of it is kept.
///
/// Neither pending set is ordered. A batch takes the columns nearest the player's position at
/// the moment it goes out, so any order stored earlier could only be a stale one — the player
/// turns, and the front of the queue is behind them.
#[derive(Component)]
pub struct ColumnView {
    /// Wanted, but not every section has landed or its light has not settled.
    awaiting_columns: FxHashSet<ColumnPos>,
    /// Ready to go out on the wire, not yet sent.
    pending_send: FxHashSet<ColumnPos>,
    /// Columns this view has asked for and holds a margin around.
    loaded_columns: FxHashSet<ColumnPos>,
    /// Forced tickets, counted: the margins of adjacent columns overlap, so a
    /// column leaving the view may not release a ticket another still needs.
    forced_columns: FxHashMap<ColumnPos, u32>,
    pub sent_columns: FxHashSet<ColumnPos>,
    batch_quota: f32,
    desired_columns_per_tick: f32,
    unacknowledged_batches: u32,
    max_unacknowledged_batches: u32,
}

impl Default for ColumnView {
    fn default() -> Self {
        Self {
            awaiting_columns: FxHashSet::default(),
            pending_send: FxHashSet::default(),
            loaded_columns: FxHashSet::default(),
            forced_columns: FxHashMap::default(),
            sent_columns: FxHashSet::default(),
            batch_quota: 0.0,
            desired_columns_per_tick: START_COLUMNS_PER_TICK,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: 1,
        }
    }
}

impl ColumnView {
    /// Whether this view already owes the player the column, at any stage.
    fn wants(&self, col: ColumnPos) -> bool {
        self.awaiting_columns.contains(&col)
            || self.pending_send.contains(&col)
            || self.sent_columns.contains(&col)
    }

    /// Takes a reference on `col`'s forced ticket, reporting whether this is
    /// the first and the ticket has to be added.
    fn hold_forced(&mut self, col: ColumnPos) -> bool {
        let holders = self.forced_columns.entry(col).or_default();
        *holders += 1;
        *holders == 1
    }

    /// Drops a reference, reporting whether it was the last and the ticket has
    /// to go.
    fn release_forced(&mut self, col: ColumnPos) -> bool {
        let std::collections::hash_map::Entry::Occupied(mut slot) = self.forced_columns.entry(col)
        else {
            return false;
        };
        *slot.get_mut() -= 1;
        if *slot.get() == 0 {
            slot.remove();
            return true;
        }
        false
    }

    fn forget(&mut self, col: ColumnPos) {
        self.awaiting_columns.remove(&col);
        self.pending_send.remove(&col);
    }

    /// The client has taken a batch and says how many columns a tick it managed while doing so.
    pub fn acknowledge_batch(&mut self, desired_columns_per_tick: f32) {
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_sub(1);
        self.desired_columns_per_tick = if desired_columns_per_tick.is_nan() {
            MIN_COLUMNS_PER_TICK
        } else {
            desired_columns_per_tick.clamp(MIN_COLUMNS_PER_TICK, MAX_COLUMNS_PER_TICK)
        };
        if self.unacknowledged_batches == 0 {
            self.batch_quota = 1.0;
        }
        self.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;
    }
}

fn handle_batch_acknowledgement(on: On<ReceivedPacketEvent>, mut players: Query<&mut ColumnView>) {
    let Some(ack) = on.decode::<ServerboundChunkBatchReceived>() else {
        return;
    };
    let Ok(mut chunk_view) = players.get_mut(on.entity) else {
        return;
    };
    chunk_view.acknowledge_batch(ack.desired_chunks_per_tick);
}

/// Takes a column into the view's want list and forces its sections to stay loaded while it is
/// there. The only writer that puts a column into `awaiting_columns`.
pub(crate) fn request_columns(
    mut message: MessageReader<PlayerChunkLoadRequest>,
    mut players: Query<(&mut ColumnView, &InDimension, &Reposition)>,
    mut dims: Query<(&mut ChunkTicketsCommands, &DimensionTypeConfig)>,
) {
    message.read().for_each(|req| {
        let Ok((mut chunk_view, dim, rep)) = players.get_mut(req.player) else {
            return;
        };
        let column_pos = req.column_pos;
        if chunk_view.wants(column_pos) {
            return;
        }
        let Ok((mut cmds, type_config)) = dims.get_mut(dim.entity()) else {
            return;
        };
        chunk_view.awaiting_columns.insert(column_pos);
        // The margin, not just the column: a column's own light is only final
        // once the eight around it hold blocks, so the ring past the view has to
        // be loaded even though it is never sent.
        if chunk_view.loaded_columns.insert(column_pos) {
            let off = offset_sections(rep, type_config.min_y);
            for col in margin_of(column_pos) {
                if chunk_view.hold_forced(col) {
                    apply_forced_tickets(&mut cmds, col, off, type_config.section_count, true);
                }
            }
        }
        trace!(
            "Player {:?} requested load of chunk column {:?}",
            req.player, column_pos
        );
    })
}

/// The forget goes out from the same view diff that sent the column, as vanilla's chunk map
/// does, so what the client holds is exactly what the server counts as sent: a forget from any
/// other radius leaves a column the server will never send again.
fn unload_chunk_request(
    mut message: MessageReader<PlayerChunkUnloadRequest>,
    mut players: Query<(&mut ColumnView, &InDimension, &Reposition, &HostAnchor)>,
    mut dims: Query<(&mut ChunkTicketsCommands, &DimensionTypeConfig)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    message.read().for_each(|req| {
        let Ok((mut chunk_view, in_dim, rep, host_anchor)) = players.get_mut(req.player) else {
            return;
        };
        let column_pos = req.column_pos;
        column_trace::forget(column_pos);
        chunk_view.forget(column_pos);
        if chunk_view.sent_columns.remove(&column_pos) {
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(host_anchor.0),
                priority: PacketPriority::Critical,
                data: PacketPayload::ChunkUnload {
                    column: ColumnPos::new(
                        rep.convert_chunk_x(column_pos.x),
                        rep.convert_chunk_z(column_pos.z),
                    ),
                },
                session: PlayerSession(0),
                epoch: 0,
            });
        }
        if chunk_view.loaded_columns.remove(&column_pos)
            && let Ok((mut cmds, type_config)) = dims.get_mut(in_dim.entity())
        {
            let off = offset_sections(rep, type_config.min_y);
            for col in margin_of(column_pos) {
                if chunk_view.release_forced(col) {
                    apply_forced_tickets(&mut cmds, col, off, type_config.section_count, false);
                }
            }
        }
    });
}

/// Only the view knows which sections it forced, so a player leaving the
/// dimension that does not hand them back here pins them loaded for good.
fn release_forced_tickets(
    remove: On<Remove, ColumnView>,
    players: Query<(&ColumnView, &InDimension, &Reposition)>,
    mut dims: Query<(&mut ChunkTicketsCommands, &DimensionTypeConfig)>,
) {
    let Ok((view, in_dim, rep)) = players.get(remove.event().entity) else {
        return;
    };
    let Ok((mut cmds, type_config)) = dims.get_mut(in_dim.entity()) else {
        return;
    };
    let off = offset_sections(rep, type_config.min_y);
    for &col in view.forced_columns.keys() {
        apply_forced_tickets(&mut cmds, col, off, type_config.section_count, false);
    }
}

/// The column entity and the sections it hands the client, in client order, or `None` while
/// the engine is still short of either. The single place a column's presence is decided, so
/// readiness and the send that follows it cannot disagree about what the column is made of.
///
/// The column entity is part of that answer, not a detail of the send: light and heightmaps
/// are read off it, and a column sent before `reconcile_columns` has indexed it would carry
/// neither. It goes out once and never again, so it would stay black.
fn resolve_column(
    chunk_index: &ChunkIndex,
    column_index: &ColumnIndex,
    col: ColumnPos,
    section_count: i32,
    off: i32,
) -> Option<(Entity, Vec<Entity>)> {
    let column = column_index
        .0
        .get(&EngineColumnPos::new(col.x, col.z))
        .map(|slot| slot.entity)?;
    let sections = (0..section_count)
        .map(|client_y| chunk_index.get(SectionPos::new(col.x, client_y - off, col.z)))
        .collect::<Option<Vec<_>>>()?;
    Some((column, sections))
}

/// Moves a column from "wanted" to "ready to send" once every section it carries has landed
/// and its light has settled. The only writer of that transition.
///
/// The light gate belongs here rather than at the send: a column goes out once and never
/// again, so sending one before its light has settled leaves it permanently black on the
/// client.
pub(crate) fn project_ready_columns(
    mut players: Query<(&mut ColumnView, &InDimension, &Reposition)>,
    dims: Query<(&ChunkIndex, &ColumnIndex, &DimensionTypeConfig)>,
    chunks: Query<Entity, With<ChunkLoaded>>,
    codec_params: LightCodecParams,
    light_status: mcrs_minecraft_light::prelude::LightStatus,
) {
    let await_light = light_status.is_installed() && !crate::lighting_disabled();
    players.iter_mut().for_each(|(mut chunk_view, dim, rep)| {
        let Ok((chunk_index, column_index, type_config)) = dims.get(dim.entity()) else {
            return;
        };
        let section_count = type_config.section_count as i32;
        let off = offset_sections(rep, type_config.min_y);
        let view = &mut *chunk_view;
        let sections_of = |col: ColumnPos| {
            resolve_column(chunk_index, column_index, col, section_count, off)
                .filter(|(_, sections)| sections.iter().all(|&e| chunks.contains(e)))
        };
        // A cell's light is decided by the blocks within fifteen of it, which
        // reaches one column out and no further. So the neighbours owe this
        // column their blocks, never their light: waiting on their light too
        // would hold every column of a bulk load behind the whole queue.
        let blocks_landed = |col: ColumnPos| sections_of(col).is_some();
        let light_landed = |col: ColumnPos| {
            sections_of(col).is_some_and(|(_, sections)| {
                !await_light
                    || sections.iter().all(|&chunk_e| {
                        codec_params.block_lights.contains(chunk_e)
                            && codec_params.sky_lights.contains(chunk_e)
                    })
            })
        };
        view.awaiting_columns.retain(|&col| {
            // Light computed against a neighbour whose blocks have not arrived
            // is a seam the column would carry for as long as the client holds
            // it, because a column goes out once. `settled_around` then waits
            // for the work those neighbours raised.
            let ready = light_landed(col) && margin_of(col).all(blocks_landed);
            if !ready || (await_light && !light_status.settled_around(col)) {
                return true;
            }
            trace!("Column {:?} ready", col);
            column_trace::mark(col, ColumnStage::Ready);
            view.pending_send.insert(col);
            false
        });
    });
}

/// The rate the client is asked to answer with, in columns a tick. The ceiling is only a guard
/// against a nonsense answer: a client on the same machine as its server measures itself able to
/// take several hundred a tick, and clamping that back to a round number throttles the load for
/// no reason the client asked for.
const MIN_COLUMNS_PER_TICK: f32 = 0.01;
const MAX_COLUMNS_PER_TICK: f32 = 4096.0;
const START_COLUMNS_PER_TICK: f32 = 9.0;

/// A ceiling on one batch, not the rate: the client's answer is what paces the load, and a cap
/// low enough to bind first silently overrides it — at a full render distance a column is some
/// seventy kilobytes on the wire, so half the socket's queue was barely a hundred of them. The
/// bridge hands a blob over in pieces and buffers what the writer cannot take, and only kills a
/// connection sixteen socket queues behind, so a batch this size leaves it several ticks of room.
const MAX_BATCH_BYTES: usize = 4 * mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET;

/// Batches allowed in flight once the client has answered one. Until then a single batch is
/// out at a time, so a client that cannot keep up is never sent a second one to prove it.
const MAX_UNACKNOWLEDGED_BATCHES: u32 = 10;

/// Squared XZ distance from a column to the centre of the player's view.
fn column_distance_sq(pos: ColumnPos, center: SectionPos) -> i64 {
    let dx = (pos.x - center.x) as i64;
    let dz = (pos.z - center.z) as i64;
    dx * dx + dz * dz
}

/// Chunk-load wire emit routed through the `OutboundPlayerPacket` bus.
///
/// The rate is the client's: each batch is bracketed by a start and a finish, and the client
/// answers with the columns a tick it managed. Chunks are sent at Critical priority so they
/// are never dropped by the bridge.
pub(crate) fn send_column_queue(
    mut players: Query<(
        &mut ColumnView,
        &PlayerChunkObserver,
        &Reposition,
        &InDimension,
        &HostAnchor,
    )>,
    chunks: Query<(&ChunkBlocks, &BiomePalette, Option<&SectionBlockEntities>), With<ChunkLoaded>>,
    block_entities_held: Query<&'static BlockEntity>,
    dim_chunk_indexes: Query<&ChunkIndex>,
    dim_column_indexes: Query<&ColumnIndex>,
    dim_type_configs: Query<&DimensionTypeConfig>,
    column_heightmaps: Query<(&SurfaceHeightmap, &MotionHeightmap, &NoLeavesHeightmap)>,
    codec_params: LightCodecParams,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    use std::sync::atomic::Ordering;
    players
        .iter_mut()
        .for_each(|(mut chunk_view, observer, rep, in_dim, host_anchor)| {
            let host = host_anchor.0;
            let Ok(chunk_index) = dim_chunk_indexes.get(in_dim.entity()) else {
                return;
            };
            let Ok(column_index) = dim_column_indexes.get(in_dim.entity()) else {
                return;
            };
            let Ok(type_config) = dim_type_configs.get(in_dim.entity()) else {
                return;
            };
            let wire_light_rows = type_config.section_count as usize + 2;
            let section_count = type_config.section_count as i32;
            let off = offset_sections(rep, type_config.min_y);

            if chunk_view.unacknowledged_batches >= chunk_view.max_unacknowledged_batches {
                return;
            }
            let desired = chunk_view.desired_columns_per_tick;
            chunk_view.batch_quota = (chunk_view.batch_quota + desired).min(desired.max(1.0));
            if chunk_view.batch_quota < 1.0 {
                return;
            }
            let allowed = chunk_view.batch_quota as usize;
            if allowed == 0 || chunk_view.pending_send.is_empty() {
                return;
            }

            // Nearest to where the player is now, not to where they were when the column came
            // ready: one step of the player invalidates any order settled earlier. The ready
            // set is bounded by the view, so taking the batch out of it costs one pass and a
            // partial selection rather than a kept ordering that goes stale on its own.
            let center = observer
                .last_last_chunk_tracking_view
                .map(|view| view.center)
                .unwrap_or(SectionPos::new(0, 0, 0));
            let mut nearest: Vec<ColumnPos> = chunk_view.pending_send.iter().copied().collect();
            if allowed < nearest.len() {
                nearest.select_nth_unstable_by_key(allowed, |pos| column_distance_sq(*pos, center));
                nearest.truncate(allowed);
            }
            nearest.sort_unstable_by_key(|pos| column_distance_sq(*pos, center));

            let mut sends = 0usize;
            let mut batch_bytes = 0usize;
            let mut batch = Vec::with_capacity(nearest.len());

            for column_pos in nearest {
                if batch_bytes >= MAX_BATCH_BYTES {
                    break;
                }

                // The forced tickets this view holds keep every section of a pending column
                // loaded, so losing one here means the ticket and the send disagree about
                // what the view owns. Send nothing rather than a column the client can never
                // be sent again to fix.
                let held =
                    resolve_column(chunk_index, column_index, column_pos, section_count, off).map(
                        |(column_entity, sections)| {
                            sections
                                .into_iter()
                                .map(|chunk_e| chunks.get(chunk_e))
                                .collect::<Result<Vec<_>, _>>()
                                .map(|sections| (column_entity, sections))
                        },
                    );
                let Some(Ok((column_entity, sections))) = held else {
                    warn!(
                        col_x = column_pos.x,
                        col_z = column_pos.z,
                        "a pending column lost a section it holds a ticket on"
                    );
                    chunk_view.pending_send.remove(&column_pos);
                    chunk_view.awaiting_columns.insert(column_pos);
                    continue;
                };

                let mut data = Vec::with_capacity(16 * 1024);
                let mut block_entities: Vec<ChunkDataBlockEntity<'static>> = Vec::new();
                for (blocks, biomes, section_block_entities) in sections {
                    for held in section_block_entities.iter().flat_map(|index| index.iter()) {
                        let Ok(BlockEntity(held)) = block_entities_held.get(*held) else {
                            continue;
                        };
                        match packet_entry(held) {
                            Ok(entry) => block_entities.push(entry),
                            Err(err) => warn!(%err, "encoding a block entity for the wire"),
                        }
                    }
                    // section and turns the rest of the column into garbage.
                    blocks
                        .non_air_block_count()
                        .encode(&mut data)
                        .expect("Failed to encode chunk block count");
                    0u16.encode(&mut data)
                        .expect("Failed to encode chunk fluid count");
                    blocks
                        .0
                        .0
                        .encode(&mut data)
                        .expect("Failed to encode chunk block data");
                    biomes
                        .0
                        .encode(&mut data)
                        .expect("Failed to encode chunk biome data");
                }
                let light_data = if crate::lighting_disabled() {
                    build_fullbright_light_data(wire_light_rows)
                } else {
                    build_full_light_data(column_entity, &codec_params)
                };

                let heightmaps = column_heightmaps
                    .get(column_entity)
                    .map(|(surface, motion, no_leaves)| {
                        client_heightmaps(surface, motion, no_leaves)
                    })
                    .unwrap_or_default();

                let wire_pos = ColumnPos::new(
                    rep.convert_chunk_x(column_pos.x),
                    rep.convert_chunk_z(column_pos.z),
                );

                column_trace::mark(column_pos, ColumnStage::Sent);
                chunk_view.pending_send.remove(&column_pos);
                chunk_view.sent_columns.insert(column_pos);

                trace!(
                    target: "mcrs_minecraft_server::player",
                    host_anchor = ?host,
                    col_x = wire_pos.x,
                    col_z = wire_pos.z,
                    bytes = data.len(),
                    "send_column_queue: emitting ChunkLoad via bus"
                );

                batch_bytes += data.len()
                    + (light_data.sky_light_arrays.len() + light_data.block_light_arrays.len())
                        * size_of::<mcrs_minecraft_protocol::chunk::LightChunk>();
                batch.push(PacketPayload::ChunkLoad {
                    column: wire_pos,
                    chunk_bytes: data,
                    heightmaps,
                    light_data,
                    block_entities,
                });

                sends += 1;
            }
            if batch.is_empty() {
                return;
            }
            chunk_view.unacknowledged_batches += 1;
            chunk_view.batch_quota -= sends as f32;

            let batch_size = batch.len() as u32;
            let mut emit = |data| {
                packet_writer.write(OutboundPlayerPacket {
                    target: PacketTarget::SinglePlayer(host),
                    priority: PacketPriority::Critical,
                    data,
                    session: PlayerSession(0),
                    epoch: 0,
                });
                mcrs_minecraft_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
                    .fetch_add(1, Ordering::Relaxed);
            };
            emit(PacketPayload::ChunkBatchStart);
            for column in batch {
                emit(column);
            }
            emit(PacketPayload::ChunkBatchFinished { batch_size });
        })
}

fn add_player_column_view(
    players: Query<Entity, Added<PlayerChunkObserver>>,
    mut commands: Commands,
) {
    for player in &players {
        commands.entity(player).insert(ColumnView::default());
    }
}

#[inline]
fn offset_sections(rep: &Reposition, min_y: i32) -> i32 {
    let bits = SectionPos::BITS as i32;
    (rep.offset_y_blocks() >> bits) - (min_y >> bits)
}

/// A column and the eight around it, or the column alone while the margin is
/// off.
fn margin_of(col: ColumnPos) -> impl Iterator<Item = ColumnPos> {
    let width = if margin_enabled() { 1 } else { 0 };
    (-width..=width)
        .flat_map(move |dz| (-width..=width).map(move |dx| ColumnPos::new(col.x + dx, col.z + dz)))
}

/// Holding the ring past the view stalls the loader as it stands, so it is off
/// until that is understood: `MCRS_MARGIN=1` turns it on.
fn margin_enabled() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var("MCRS_MARGIN").as_deref() == Ok("1"));
    *ON
}

fn apply_forced_tickets(
    tickets: &mut ChunkTicketsCommands,
    col: ColumnPos,
    off_sections: i32,
    section_count: u32,
    add: bool,
) {
    for client_y in 0..section_count as i32 {
        let server_y = client_y - off_sections;
        let chunk_pos = SectionPos::new(col.x, server_y, col.z);
        if add {
            tickets.add_ticket(chunk_pos, Ticket::new(TicketKind::Forced));
        } else {
            tickets.remove_ticket(chunk_pos, TicketKind::Forced);
        }
    }
}

/// Handles view updates:
/// - sends cache center / radius via the `OutboundPlayerPacket` bus
/// - diffs column set (xz only)
/// - adds/removes Forced tickets for the whole client column window (16 sections, mapped by Reposition)
/// - enqueues load/unload
fn on_view_update(
    event: On<ChunkTrackingViewUpdateEvent>,
    q: Query<(&Reposition, &HostAnchor)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    use std::sync::atomic::Ordering;
    let Ok((rep, host_anchor)) = q.get(event.player) else {
        return;
    };
    let host = host_anchor.0;
    trace!(
        "Player {:?} chunk view updated: old={:?} new={:?}",
        event.player, event.old_view, event.new_view
    );

    // Cache center / radius routed through the bus (Critical: required for chunk rendering).
    if match event.old_view {
        Some(a) => a.center != event.new_view.center,
        None => true,
    } {
        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::SetChunkCacheCenter(ColumnPos::new(
                rep.convert_chunk_x(event.new_view.center.x),
                rep.convert_chunk_z(event.new_view.center.z),
            )),
            session: PlayerSession(0),
            epoch: 0,
        });
        mcrs_minecraft_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);
    }
    if match event.old_view {
        Some(v) => v.distance != event.new_view.distance,
        None => true,
    } {
        packet_writer.write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(host),
            priority: PacketPriority::Critical,
            data: PacketPayload::SetChunkCacheRadius {
                radius: event.new_view.distance as i32,
            },
            session: PlayerSession(0),
            epoch: 0,
        });
        mcrs_minecraft_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL
            .fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_minecraft_light::prelude::{
        BlockLight, LightBounds, LightProperties, LightRegistry, LightWorld, SkyLight,
        SpecialBlocks,
    };
    use mcrs_voxel_world::world::dimension::HasSkyLight;
    use mcrs_voxel_world::world::storage::column::{ColumnChunks, ColumnSlot};
    use rustc_hash::FxHashMap;

    const SECTIONS: u32 = 2;

    struct Fixture {
        dim: Entity,
        player: Entity,
        sections: FxHashMap<ColumnPos, Vec<Entity>>,
    }

    /// A dimension holding `columns`, each with every section loaded, and one player looking at
    /// the origin. Sections carry no light yet.
    fn fixture(world: &mut World, columns: &[ColumnPos]) -> Fixture {
        let dim = world.spawn_empty().id();
        let mut chunk_index = ChunkIndex::new();
        let mut column_index = ColumnIndex::default();
        let mut sections = FxHashMap::default();

        for &col in columns {
            let entities: Vec<Entity> = (0..SECTIONS as i32)
                .map(|y| {
                    let section = world
                        .spawn((ChunkBlocks::default(), BiomePalette::default(), ChunkLoaded))
                        .id();
                    chunk_index.insert(SectionPos::new(col.x, y, col.z), section);
                    section
                })
                .collect();
            let column = world
                .spawn((
                    ColumnChunks {
                        min_section_y: 0,
                        sections: entities.iter().copied().map(Some).collect(),
                    },
                    InDimension(dim),
                ))
                .id();
            column_index.0.insert(
                EngineColumnPos::new(col.x, col.z),
                ColumnSlot {
                    entity: column,
                    section_count: SECTIONS,
                },
            );
            sections.insert(col, entities);
        }

        world.entity_mut(dim).insert((
            DimensionTypeConfig::new(0, SECTIONS << 4),
            HasSkyLight,
            ChunkTicketsCommands::default(),
            chunk_index,
            column_index,
        ));

        let host = world.spawn_empty().id();
        let player = world
            .spawn((
                ColumnView::default(),
                PlayerChunkObserver::default(),
                Reposition::default(),
                InDimension(dim),
                HostAnchor(host),
            ))
            .id();

        let registry = std::sync::Arc::new(LightRegistry::new(
            vec![LightProperties::AIR, LightProperties::SOLID],
            SpecialBlocks {
                unloaded: mcrs_voxel_storage::VoxelId(1),
                outside: mcrs_voxel_storage::VoxelId(0),
            },
        ));
        world.insert_resource(mcrs_minecraft_light::prelude::Lighting(LightWorld::new(
            registry,
            LightBounds::new(0, SECTIONS as i32 - 1),
        )));
        world.init_resource::<Messages<OutboundPlayerPacket>>();

        Fixture {
            dim,
            player,
            sections,
        }
    }

    fn light(world: &mut World, sections: &[Entity]) {
        for &section in sections {
            world
                .entity_mut(section)
                .insert((BlockLight::default(), SkyLight::default()));
        }
    }

    fn sent_columns(world: &mut World) -> Vec<ColumnPos> {
        world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .filter_map(|packet| match packet.data {
                PacketPayload::ChunkLoad { column, .. } => Some(column),
                _ => None,
            })
            .collect()
    }

    /// The whole point of holding the ready columns unordered: the batch is chosen against
    /// where the player is when it goes out, so the nearest column always leads it however the
    /// columns happened to arrive.
    #[test]
    fn a_batch_goes_out_nearest_first() {
        let mut world = World::new();
        let far = ColumnPos::new(5, 0);
        let near = ColumnPos::new(1, 0);
        let middle = ColumnPos::new(0, 3);
        let fx = fixture(&mut world, &[far, near, middle]);
        for sections in fx.sections.values() {
            let sections = sections.clone();
            light(&mut world, &sections);
        }

        let mut view = world.get_mut::<ColumnView>(fx.player).unwrap();
        view.desired_columns_per_tick = 3.0;
        for col in [far, near, middle] {
            view.pending_send.insert(col);
        }

        world
            .run_system_once(send_column_queue)
            .expect("the send runs");

        assert_eq!(sent_columns(&mut world), vec![near, middle, far]);
    }

    /// A column is sent once and never again, so one that goes out before its light has settled
    /// is permanently black on the client. Readiness, not the send, is where that is decided.
    #[test]
    fn a_column_waits_for_the_light_of_every_section_it_carries() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        // Readiness reads the whole neighbourhood, so the margin is lit up
        // front and only the column's own sections are left to arrive.
        let neighbourhood: Vec<ColumnPos> = margin_of(col).collect();
        let fx = fixture(&mut world, &neighbourhood);
        for &neighbour in &neighbourhood {
            if neighbour != col {
                let lit = fx.sections[&neighbour].clone();
                light(&mut world, &lit);
            }
        }
        let sections = fx.sections[&col].clone();

        world
            .get_mut::<ColumnView>(fx.player)
            .unwrap()
            .awaiting_columns
            .insert(col);

        for (i, &section) in sections.iter().enumerate() {
            world
                .run_system_once(project_ready_columns)
                .expect("the projection runs");
            assert!(
                world
                    .get::<ColumnView>(fx.player)
                    .unwrap()
                    .pending_send
                    .is_empty(),
                "{} of {} sections lit",
                i,
                sections.len()
            );
            light(&mut world, &[section]);
        }

        world
            .run_system_once(project_ready_columns)
            .expect("the projection runs");
        assert!(
            world
                .get::<ColumnView>(fx.player)
                .unwrap()
                .pending_send
                .contains(&col)
        );
    }

    #[test]
    fn a_despawned_view_hands_back_its_forced_tickets() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let fx = fixture(&mut world, &[col]);
        world.add_observer(release_forced_tickets);
        world.init_resource::<Messages<PlayerChunkLoadRequest>>();
        world
            .resource_mut::<Messages<PlayerChunkLoadRequest>>()
            .write(PlayerChunkLoadRequest {
                player: fx.player,
                column_pos: col,
            });
        world
            .run_system_once(request_columns)
            .expect("the request runs");

        let queued = |world: &World| -> usize {
            let tickets = world.get::<ChunkTicketsCommands>(fx.dim).unwrap();
            (0..SECTIONS as i32)
                .map(|y| {
                    tickets
                        .queued_tickets(SectionPos::new(col.x, y, col.z))
                        .len()
                })
                .sum()
        };
        assert_eq!(queued(&world), SECTIONS as usize);

        world.despawn(fx.player);

        assert_eq!(queued(&world), 0);
    }

    /// The forced tickets make this unreachable, so the guard is what stops a broken
    /// invariant from putting half a column on the wire.
    #[test]
    fn a_column_missing_a_section_goes_back_to_waiting_instead_of_out() {
        let mut world = World::new();
        let col = ColumnPos::new(0, 0);
        let fx = fixture(&mut world, &[col]);
        let sections = fx.sections[&col].clone();
        light(&mut world, &sections);

        world
            .get_mut::<ColumnView>(fx.player)
            .unwrap()
            .pending_send
            .insert(col);
        world
            .get_mut::<ChunkIndex>(fx.dim)
            .unwrap()
            .remove(SectionPos::new(col.x, 1, col.z));

        world
            .run_system_once(send_column_queue)
            .expect("the send runs");

        assert!(sent_columns(&mut world).is_empty());
        let view = world.get::<ColumnView>(fx.player).unwrap();
        assert!(view.pending_send.is_empty());
        assert!(view.awaiting_columns.contains(&col));
    }
}
