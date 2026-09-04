use std::collections::VecDeque;

use bevy_app::{App, FixedUpdate, Plugin, PreUpdate};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{
    Added, Component, ContainsEntity, Message, MessageReader, On, Query, With,
};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::Commands;
use mcrs_minecraft_block::palette::{AirCount, BiomePalette, BlockPalette, NetworkPalette};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::light_codec::{
    LightCodecParams, build_full_light_data, build_fullbright_light_data,
};
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChunkBatchReceived;
use mcrs_minecraft_protocol::{ColumnPos, Encode};
use mcrs_voxel_math::ChunkPos;
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
use mcrs_voxel_world::world::storage::chunk::ChunkIndex;
use mcrs_voxel_world::world::storage::column::{ColumnIndex, ColumnPos as EngineColumnPos};

use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;
use rustc_hash::FxHashSet;
use tracing::trace;

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
                load_chunk_request,
                load_column_queue,
                loading_column_queue,
                send_column_queue,
            )
                .chain()
                .in_set(ColumnViewSet),
        );

        // React to ChunkTrackingView changes (xz-distance changes, movement).
        app.add_observer(on_view_update);
        app.add_observer(handle_batch_acknowledgement);

        // When vertical reposition offset changes, re-map forced tickets and re-send active columns.
        // app.add_systems(Update, handle_reposition_changed);

        // Progressively ticket columns closest to the player, then send loaded ones.
        // app.add_systems(
        //     FixedUpdate,
        //     (ticket_pending_columns, process_column_queues).chain(),
        // );
    }
}

#[derive(Component)]
pub struct ColumnView {
    desired_columns: FxHashSet<ColumnPos>,
    loaded_columns: FxHashSet<ColumnPos>,
    load_queue: VecDeque<ColumnPos>,
    loading_queue: VecDeque<ColumnPos>,
    send_queue: VecDeque<(ColumnPos, Vec<Entity>)>,
    pub sent_columns: FxHashSet<ColumnPos>,
    batch_quota: f32,
    desired_columns_per_tick: f32,
    unacknowledged_batches: u32,
    max_unacknowledged_batches: u32,
}

impl Default for ColumnView {
    fn default() -> Self {
        Self {
            desired_columns: FxHashSet::default(),
            loaded_columns: FxHashSet::default(),
            load_queue: VecDeque::new(),
            loading_queue: VecDeque::new(),
            send_queue: VecDeque::new(),
            sent_columns: FxHashSet::default(),
            batch_quota: 0.0,
            desired_columns_per_tick: START_COLUMNS_PER_TICK,
            unacknowledged_batches: 0,
            max_unacknowledged_batches: 1,
        }
    }
}

impl ColumnView {
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

pub(crate) fn load_chunk_request(
    mut message: MessageReader<PlayerChunkLoadRequest>,
    mut players: Query<&mut ColumnView>,
) {
    message.read().for_each(|req| {
        let Ok(mut chunk_view) = players.get_mut(req.player) else {
            return;
        };
        let column_pos = ColumnPos::from(req.chunk_pos);
        if chunk_view.sent_columns.contains(&column_pos) {
            return;
        }

        if chunk_view.desired_columns.insert(column_pos) {
            trace!(
                "Player {:?} requested load of chunk column {:?}",
                req.player, column_pos
            );
            chunk_view.load_queue.push_back(column_pos);
        }
    })
}

/// The forget goes out from the same view diff that sent the column, as vanilla's chunk map
/// does, so what the client holds is exactly what the server counts as sent: a forget from any
/// other radius leaves a column the server will never send again.
fn unload_chunk_request(
    mut message: MessageReader<PlayerChunkUnloadRequest>,
    mut players: Query<(
        &mut ColumnView,
        &InDimension,
        &Reposition,
        &PlayerChunkObserver,
        &HostAnchor,
    )>,
    mut dims: Query<(&mut ChunkTicketsCommands, &DimensionTypeConfig)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    message.read().for_each(|req| {
        let Ok((mut chunk_view, in_dim, rep, observer, host_anchor)) = players.get_mut(req.player)
        else {
            return;
        };
        let column_pos = ColumnPos::from(req.chunk_pos);
        // The queue evicts one section at a time, but a column is sent and
        // dropped whole. Crossing a section boundary vertically evicts one
        // section from every column in view, and tearing those columns down
        // for it would re-send the entire view.
        if observer
            .last_last_chunk_tracking_view
            .is_some_and(|view| view.contains_column(column_pos.x, column_pos.z))
        {
            return;
        }
        column_trace::forget(column_pos);
        chunk_view.desired_columns.remove(&column_pos);
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
            apply_forced_tickets(
                &mut cmds,
                column_pos,
                offset_sections(rep, type_config.min_y),
                type_config.section_count,
                false,
            );
        }
    });
}

pub(crate) fn load_column_queue(
    mut players: Query<(&mut ColumnView, &InDimension, &Reposition)>,
    mut dims: Query<(&mut ChunkTicketsCommands, &DimensionTypeConfig)>,
) {
    players.iter_mut().for_each(|(mut chunk_view, dim, rep)| {
        let Ok((mut cmds, type_config)) = dims.get_mut(dim.0) else {
            return;
        };
        let section_count = type_config.section_count;
        while let Some(col) = chunk_view.load_queue.pop_front() {
            if chunk_view.desired_columns.contains(&col) {
                if chunk_view.loaded_columns.insert(col) {
                    apply_forced_tickets(
                        &mut cmds,
                        col,
                        offset_sections(rep, type_config.min_y),
                        section_count,
                        true,
                    );
                    trace!("Added tickets to col: {:?}", col);
                }
                chunk_view.loading_queue.push_back(col);
            }
        }
    })
}

pub(crate) fn loading_column_queue(
    mut players: Query<(&mut ColumnView, &InDimension, &Reposition)>,
    dims: Query<(&ChunkIndex, &DimensionTypeConfig)>,
    chunks: Query<Entity, With<ChunkLoaded>>,
) {
    players.iter_mut().for_each(|(mut chunk_view, dim, rep)| {
        let Ok((chunk_index, type_config)) = dims.get(dim.entity()) else {
            return;
        };
        let section_count = type_config.section_count as i32;
        let off = offset_sections(rep, type_config.min_y);
        // A column that is still loading must not hold back the ones behind it: a column read
        // from the save lands in a fraction of a millisecond while a generated one takes
        // several ticks, and they share this queue.
        let ColumnView {
            loading_queue,
            send_queue,
            desired_columns,
            ..
        } = &mut *chunk_view;
        loading_queue.retain(|&col| {
            if !desired_columns.contains(&col) {
                return false;
            }
            let mut chunks_entities = Vec::with_capacity(section_count as usize);
            for client_y in 0..section_count {
                let server_y = client_y - off;
                let pos = ChunkPos::new(col.x, server_y, col.z);
                match chunk_index.get(pos) {
                    Some(chunk_e) if chunks.contains(chunk_e) => chunks_entities.push(chunk_e),
                    _ => return true,
                }
            }
            trace!("Column {:?} loaded", col);
            column_trace::mark(col, ColumnStage::Ready);
            send_queue.push_back((col, chunks_entities));
            false
        });
    })
}

/// The rate the client is asked to answer with, in columns a tick.
const MIN_COLUMNS_PER_TICK: f32 = 0.01;
const MAX_COLUMNS_PER_TICK: f32 = 96.0;
const START_COLUMNS_PER_TICK: f32 = 9.0;

/// The bridge coalesces a tick's packets into one blob and closes a connection whose blob
/// passes its cap, so a batch stops well short of it and the quota it did not spend rides to
/// the next tick.
const MAX_BATCH_BYTES: usize = mcrs_minecraft_network::MAX_QUEUED_BYTES_PER_SOCKET / 2;

/// Batches allowed in flight once the client has answered one. Until then a single batch is
/// out at a time, so a client that cannot keep up is never sent a second one to prove it.
const MAX_UNACKNOWLEDGED_BATCHES: u32 = 10;

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
    chunks: Query<(&BlockPalette, &BiomePalette), With<ChunkLoaded>>,
    dim_column_indexes: Query<&ColumnIndex>,
    dim_type_configs: Query<&DimensionTypeConfig>,
    codec_params: LightCodecParams,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    use std::sync::atomic::Ordering;
    players
        .iter_mut()
        .for_each(|(mut chunk_view, observer, rep, in_dim, host_anchor)| {
            let host = host_anchor.0;
            let column_index = dim_column_indexes.get(in_dim.entity()).ok();
            let wire_light_rows = dim_type_configs
                .get(in_dim.entity())
                .map(|config| config.section_count as usize + 2)
                .unwrap_or(0);
            if chunk_view.unacknowledged_batches >= chunk_view.max_unacknowledged_batches {
                return;
            }
            let desired = chunk_view.desired_columns_per_tick;
            chunk_view.batch_quota = (chunk_view.batch_quota + desired).min(desired.max(1.0));
            if chunk_view.batch_quota < 1.0 {
                return;
            }
            let allowed = chunk_view.batch_quota as usize;
            let mut sends = 0usize;
            let mut batch_bytes = 0usize;
            let mut batch = Vec::with_capacity(allowed);

            // A column the view has since dropped would spend a batch on terrain the player
            // has already flown past, and one further out would spend it ahead of the ground
            // under their feet, so the queue is cut down to what is still wanted and taken
            // nearest first.
            let center = observer
                .last_last_chunk_tracking_view
                .map(|view| view.center)
                .unwrap_or(ChunkPos::new(0, 0, 0));
            {
                let view = &mut *chunk_view;
                let desired = &view.desired_columns;
                view.send_queue.retain(|(pos, _)| desired.contains(pos));
                view.send_queue.make_contiguous().sort_by_key(|(pos, _)| {
                    let dx = (pos.x - center.x) as i64;
                    let dz = (pos.z - center.z) as i64;
                    dx * dx + dz * dz
                });
            }

            let mut index = 0usize;
            loop {
                if sends >= allowed || batch_bytes >= MAX_BATCH_BYTES {
                    break;
                }

                let Some((column_pos, chunks_e)) = chunk_view.send_queue.get(index) else {
                    break;
                };
                let column_pos = *column_pos;
                let mut ready = true;
                let mut data = Vec::with_capacity(16 * 1024);

                for &chunk_e in chunks_e {
                    let Ok((blocks, biomes)) = chunks.get(chunk_e) else {
                        ready = false;
                        break;
                    };
                    // Section layout per vanilla LevelChunkSection.write:
                    //   short non_empty_block_count
                    //   short fluid_count
                    //   PalettedContainer<BlockState>
                    //   PalettedContainer<Biome>
                    //
                    // The client reads both shorts unconditionally; omitting the
                    // fluid count desynchronises the reader by two bytes per
                    // section and turns the rest of the column into garbage.
                    blocks
                        .non_air_block_count()
                        .encode(&mut data)
                        .expect("Failed to encode chunk block count");
                    0u16.encode(&mut data)
                        .expect("Failed to encode chunk fluid count");
                    blocks
                        .convert_network()
                        .encode(&mut data)
                        .expect("Failed to encode chunk block data");
                    biomes
                        .convert_network()
                        .encode(&mut data)
                        .expect("Failed to encode chunk block data");
                }
                if !ready {
                    // Its chunks have not landed yet; the ones behind it may have, and the
                    // batch is worth more spent on them than on waiting.
                    index += 1;
                    continue;
                }

                let light_data = if crate::lighting_disabled() {
                    build_fullbright_light_data(wire_light_rows)
                } else {
                    column_index
                        .and_then(|idx| {
                            idx.0
                                .get(&EngineColumnPos::new(column_pos.x, column_pos.z))
                                .map(|slot| slot.entity)
                        })
                        .map(|column_entity| build_full_light_data(column_entity, &codec_params))
                        .unwrap_or_default()
                };

                let wire_pos = ColumnPos::new(
                    rep.convert_chunk_x(column_pos.x),
                    rep.convert_chunk_z(column_pos.z),
                );

                column_trace::mark(column_pos, ColumnStage::Sent);
                chunk_view.send_queue.remove(index);
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
                    light_data,
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

/// Bridges codec-emitted `ColumnLightUpdate` messages to per-player wire
/// dispatch through the `OutboundPlayerPacket` bus.
///
/// The `ColumnView::sent_columns` set gates the ordering: a light update is
/// only forwarded to a player after that player has already received the
/// corresponding `ChunkLoad` packet. Emitted at Normal priority (light
/// updates after first send can be dropped on congestion without client
/// visible loss — only the initial chunk light is Critical).
#[derive(Debug, Message)]
pub struct PlayerColumnLoadRequest {
    pub player: Entity,
    pub column_pos: ColumnPos,
    /// Server chunk entities in **client section order** (index 0..15 == client Y sections).
    pub sections: Vec<Entity>,
}

#[derive(Debug, Message)]
pub struct PlayerColumnUnloadRequest {
    pub player: Entity,
    pub column_pos: ColumnPos,
}

#[derive(Component, Default)]
pub struct PlayerColumnView {
    /// Columns the player should currently have (xz only).
    desired_columns: FxHashSet<ColumnPos>,

    /// Columns that have already been sent at least once (xz only).
    sent_columns: FxHashSet<ColumnPos>,

    /// Prevent duplicate enqueues.
    queued_columns: FxHashSet<ColumnPos>,

    /// Columns for which forced tickets have been added (chunk spawning requested).
    ticketed_columns: FxHashSet<ColumnPos>,

    /// Columns pending (re)send.
    load_queue: VecDeque<ColumnPos>,

    /// Columns pending unload.
    unload_queue: VecDeque<ColumnPos>,

    /// Last applied vertical offset, in chunk-sections (blocks >> 4).
    last_offset_sections: i32,
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
    let bits = mcrs_voxel_math::chunk_pos::BLOCKS::BITS as i32;
    (rep.offset_y_blocks() >> bits) - (min_y >> bits)
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
        let chunk_pos = ChunkPos::new(col.x, server_y, col.z);
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
            data: PacketPayload::SetChunkCacheCenter {
                x: rep.convert_chunk_x(event.new_view.center.x),
                z: rep.convert_chunk_z(event.new_view.center.z),
            },
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

    // // Compute new desired columns set.
    // let mut new_cols = FxHashSet::default();
    // columns_for_view(&event.new_view, &mut new_cols);
    //
    // // Removed columns — un-ticket only those that were actually ticketed.
    // for col in &col_view.desired_columns {
    //     if !new_cols.contains(col) {
    //         apply_forced_tickets(&mut tickets, *col, new_off, false);
    //         if col_view.sent_columns.contains(&col) {
    //             col_view.unload_queue.push_back(*col);
    //         }
    //         col_view.sent_columns.remove(&col);
    //         col_view.queued_columns.remove(&col);
    //     }
    // }
    //
    // // Added columns — only enqueue; tickets are added progressively by
    // // `ticket_pending_columns` so close chunks are generated first.
    // let center = ColumnPos::from(event.new_view.center);
    // let mut load_queue = Vec::with_capacity(new_cols.len());
    // for col in new_cols.iter() {
    //     if !col_view.desired_columns.contains(col) {
    //         if col_view.queued_columns.insert(*col) {
    //             load_queue.push(*col);
    //         }
    //     }
    // }
    // load_queue.sort_unstable_by_key(|col| col.distance_squared(center));
    // col_view.load_queue.extend(load_queue);
    //
    // col_view.desired_columns = new_cols;
}

// /// When `Reposition` changes (vertical window shifts), update forced tickets for all active columns and re-send them.
// fn handle_reposition_changed(
//     mut players: Query<
//         (
//             &Reposition,
//             &InDimension,
//             &mut PlayerColumnView,
//             &mut PlayerChunkObserver,
//         ),
//         Changed<Reposition>,
//     >,
//     mut dimensions: Query<&mut ChunkTicketsCommands>,
// ) {
//     for (rep, dim, mut view, mut observer) in &mut players {
//         let Ok(mut tickets) = dimensions.get_mut(dim.entity()) else {
//             continue;
//         };
//         let view = &mut *view;
//         let new_off = offset_sections(rep);
//         let old_off = view.last_offset_sections;
//
//         if new_off == old_off {
//             continue;
//         }
//
//         // Remap forced tickets for every currently desired column.
//         for col in (&view.desired_columns).iter() {
//             // Remove old mapping.
//             apply_forced_tickets(&mut tickets, *col, old_off, false);
//             // Add new mapping.
//             apply_forced_tickets(&mut tickets, *col, new_off, true);
//
//             // Re-send column to client (overwrites sections in-place).
//             if view.queued_columns.insert(*col) {
//                 view.load_queue.push_back(*col);
//             }
//         }
//
//         view.last_offset_sections = new_off;
//     }
// }
