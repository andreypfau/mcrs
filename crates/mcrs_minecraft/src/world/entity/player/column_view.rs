use std::collections::VecDeque;

use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin, PreUpdate};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Added, Changed, Component, ContainsEntity, Message, MessageReader, On, Or, Query, With,
};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Res};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::chunk_view::{
    ChunkTrackingView, ChunkTrackingViewUpdateEvent, ChunkViewPlugin, PlayerChunkLoadRequest,
    PlayerChunkObserver, PlayerChunkUnloadRequest,
};
use mcrs_engine::entity::player::reposition::{Reposition, RepositionConfig};
use mcrs_engine::world::chunk::ticket::{ChunkTicketsCommands, Ticket, TicketKind};
use mcrs_engine::world::chunk::{ChunkIndex, ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{
    ColumnPos as EngineColumnPos, ColumnIndex,
};
use mcrs_engine::world::dimension::{DimensionTypeConfig, InDimension};
use mcrs_minecraft_lighting::codec::{build_full_light_data, ColumnLightUpdate, LightCodecParams};
use mcrs_minecraft_lighting::components::{BlockBfsPending, SkyBfsPending};
use mcrs_minecraft_lighting::sets::LightingSet;
use mcrs_network::ServerSideConnection;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundLevelChunkWithLight, ClientboundLightUpdate,
    ClientboundSetChunkCacheCenter,
};
use mcrs_protocol::{ColumnPos, ChunkData, Encode, LightData, VarInt, WritePacket};
use rustc_hash::FxHashSet;
use tracing::{debug, info, trace};

pub struct ColumnViewPlugin;

impl Plugin for ColumnViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ChunkViewPlugin);

        // Initialize per-player column state.
        app.add_systems(PreUpdate, add_player_column_view);

        // `send_column_queue` reads `BlockLight`/`SkyLight` storage to build the
        // first-send light data via `build_full_light_data`. Without an explicit
        // ordering, the Bevy scheduler may run it before `LightingSet::Converge`
        // writes storage for a chunk that just loaded this tick — the player
        // receives a `LevelChunkWithLight` packet with zero light values and
        // never recovers (subsequent `ColumnLightUpdate` deltas only fire on
        // `Changed<*Light>`, which doesn't trigger if the section converged
        // before the first send). Anchor the chain after `EmitDirty` so the
        // initial send always sees converged storage.
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
                .after(LightingSet::EmitDirty),
        );

        app.add_systems(
            FixedPostUpdate,
            send_light_updates.after(LightingSet::Codec),
        );

        // React to ChunkTrackingView changes (xz-distance changes, movement).
        app.add_observer(on_view_update);

        // When vertical reposition offset changes, re-map forced tickets and re-send active columns.
        // app.add_systems(Update, handle_reposition_changed);

        // Progressively ticket columns closest to the player, then send loaded ones.
        // app.add_systems(
        //     FixedUpdate,
        //     (ticket_pending_columns, process_column_queues).chain(),
        // );
    }
}

#[derive(Component, Default)]
pub struct ColumnView {
    desired_columns: FxHashSet<ColumnPos>,
    loaded_columns: FxHashSet<ColumnPos>,
    load_queue: VecDeque<ColumnPos>,
    loading_queue: VecDeque<ColumnPos>,
    send_queue: VecDeque<(ColumnPos, Vec<Entity>)>,
    pub sent_columns: FxHashSet<ColumnPos>,
}

fn load_chunk_request(
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

fn unload_chunk_request(
    mut message: MessageReader<PlayerChunkUnloadRequest>,
    mut players: Query<(&mut ColumnView, &InDimension, &Reposition)>,
    mut dims: Query<(&mut ChunkTicketsCommands, &DimensionTypeConfig)>,
) {
    message.read().for_each(|req| {
        let Ok((mut chunk_view, in_dim, rep)) = players.get_mut(req.player) else {
            return;
        };
        let column_pos = ColumnPos::from(req.chunk_pos);
        chunk_view.desired_columns.remove(&column_pos);
        chunk_view.sent_columns.remove(&column_pos);
        if chunk_view.loaded_columns.remove(&column_pos) {
            if let Ok((mut cmds, type_config)) = dims.get_mut(in_dim.entity()) {
                apply_forced_tickets(&mut cmds, column_pos, offset_sections(rep, type_config.min_y), type_config.section_count, false);
            }
        }
    });
}

fn load_column_queue(
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
                    apply_forced_tickets(&mut cmds, col, offset_sections(rep, type_config.min_y), section_count, true);
                    trace!("Added tickets to col: {:?}", col);
                }
                chunk_view.loading_queue.push_back(col);
            }
        }
        true;
    })
}

fn loading_column_queue(
    mut players: Query<(&mut ColumnView, &InDimension, &Reposition)>,
    dims: Query<(&ChunkIndex, &DimensionTypeConfig)>,
    chunks: Query<Entity, With<ChunkLoaded>>,
) {
    players.iter_mut().for_each(|(mut chunk_view, dim, rep)| {
        let Ok((chunk_index, type_config)) = dims.get(dim.entity()) else {
            return;
        };
        let section_count = type_config.section_count as i32;
        loop {
            let Some(col) = chunk_view.loading_queue.front().copied() else {
                return;
            };
            if !chunk_view.desired_columns.contains(&col) {
                chunk_view.loading_queue.pop_front();
                continue;
            }
            // Check if all chunks in the column are loaded.
            let off = offset_sections(rep, type_config.min_y);
            let mut chunks_entities = Vec::with_capacity(section_count as usize);
            for client_y in 0..section_count {
                let server_y = client_y - off;
                let pos = ChunkPos::new(col.x, server_y, col.z);
                let Some(chunk_e) = chunk_index.get(pos) else {
                    return;
                };
                if chunks.contains(chunk_e) {
                    chunks_entities.push(chunk_e);
                } else {
                    return;
                }
            }
            trace!("Column {:?} loaded", col);
            chunk_view.loading_queue.pop_front();
            chunk_view.send_queue.push_back((col, chunks_entities));
        }
    })
}

/// Maximum chunk columns to send per player per tick.
const MAX_COL_SENDS_PER_TICK: usize = 10;

/// If more than this many bytes are queued for a connection, skip sending
/// more chunks this tick and let the writer task drain first.
const CHUNK_BACKPRESSURE_BYTES: usize = 2 * 1024 * 1024;

fn send_column_queue(
    mut players: Query<(
        &mut ServerSideConnection,
        &mut ColumnView,
        &Reposition,
        &InDimension,
    )>,
    chunks: Query<(&BlockPalette, &BiomePalette), With<ChunkLoaded>>,
    dim_column_indexes: Query<&ColumnIndex>,
    // Sections still cascading light through the bounded converge loop carry
    // `BlockBfsPending` or `SkyBfsPending`. Defer first-send of any column
    // with at least one such section — otherwise the codec snapshots default
    // `Mixed(NibbleArray::zeros())` from pre-converged storage and the client
    // caches darkness. The column re-enters the queue on the next tick once
    // propagation drains.
    light_dirty: Query<(), Or<(With<BlockBfsPending>, With<SkyBfsPending>)>>,
    codec_params: LightCodecParams,
) {
    players
        .iter_mut()
        .for_each(|(mut con, mut chunk_view, rep, in_dim)| {
            let column_index = dim_column_indexes.get(in_dim.entity()).ok();
            let mut sends = 0usize;

            loop {
                if sends >= MAX_COL_SENDS_PER_TICK {
                    break;
                }
                if con.queued_bytes() > CHUNK_BACKPRESSURE_BYTES {
                    break;
                }

                let Some((column_pos, chunks_e)) = chunk_view.send_queue.front() else {
                    break;
                };
                if !chunk_view.desired_columns.contains(&column_pos) {
                    chunk_view.send_queue.pop_front();
                    continue;
                }
                let column_pos = *column_pos;
                let mut ready = true;
                let mut data = Vec::with_capacity(16 * 1024);

                for &chunk_e in chunks_e {
                    let Ok((blocks, biomes)) = chunks.get(chunk_e) else {
                        ready = false;
                        break;
                    };
                    if light_dirty.contains(chunk_e) {
                        ready = false;
                        break;
                    }
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
                    // Entity data not available yet — stop processing
                    // this tick and retry on the next one. Do NOT continue,
                    // as that would re-front the same failing column in a
                    // tight loop.
                    break;
                }

                let light_data = column_index
                    .and_then(|idx| {
                        idx.0
                            .get(&EngineColumnPos::new(column_pos.x, column_pos.z))
                            .map(|slot| slot.entity)
                    })
                    .map(|column_entity| build_full_light_data(column_entity, &codec_params))
                    .unwrap_or_default();

                chunk_view.send_queue.pop_front();
                chunk_view.sent_columns.insert(column_pos);
                let pkt = ClientboundLevelChunkWithLight {
                    pos: ColumnPos::new(
                        rep.convert_chunk_x(column_pos.x),
                        rep.convert_chunk_z(column_pos.z),
                    ),
                    chunk_data: ChunkData {
                        data: data.as_slice(),
                        ..Default::default()
                    },
                    light_data,
                };
                con.write_packet(&pkt);
                sends += 1;
            }
        })
}

/// Bridges codec-emitted `ColumnLightUpdate` messages to per-player wire
/// dispatch. The `ColumnView::sent_columns` set gates the ordering: a light
/// update is only forwarded to a player after that player has already
/// received the corresponding `ClientboundLevelChunkWithLight`.
pub(crate) fn send_light_updates(
    mut reader: MessageReader<ColumnLightUpdate>,
    mut players: Query<(&ColumnView, &mut ServerSideConnection)>,
) {
    for msg in reader.read() {
        let col_pos = ColumnPos::new(msg.column_pos.x, msg.column_pos.z);
        for (view, mut con) in players.iter_mut() {
            if !view.sent_columns.contains(&col_pos) {
                continue;
            }
            let pkt = ClientboundLightUpdate {
                x: VarInt(col_pos.x),
                z: VarInt(col_pos.z),
                light_data: msg.light_data.clone(),
            };
            con.write_packet(&pkt);
        }
    }
}

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
    let bits = mcrs_engine::world::chunk::BLOCKS::BITS as i32;
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
/// - sends cache center / radius
/// - diffs column set (xz only)
/// - adds/removes Forced tickets for the whole client column window (16 sections, mapped by Reposition)
/// - enqueues load/unload
fn on_view_update(
    event: On<ChunkTrackingViewUpdateEvent>,
    mut q: Query<(&mut ServerSideConnection, &Reposition)>,
) {
    let Ok((mut con, rep)) = q.get_mut(event.player) else {
        return;
    };
    trace!(
        "Player {:?} chunk view updated: old={:?} new={:?}",
        event.player, event.old_view, event.new_view
    );

    // Cache center / radius (vanilla packets).
    if match event.old_view {
        Some(a) => a.center != event.new_view.center,
        None => true,
    } {
        con.write_packet(&ClientboundSetChunkCacheCenter {
            x: VarInt(rep.convert_chunk_x(event.new_view.center.x)),
            z: VarInt(rep.convert_chunk_z(event.new_view.center.z)),
        });
    }
    if match event.old_view {
        Some(v) => v.distance != event.new_view.distance,
        None => true,
    } {
        con.write_packet(&ClientboundChunkCacheRadius {
            radius: VarInt(event.new_view.distance as i32),
        });
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

// /// Progressively add forced tickets for columns at the front of the load queue.
// /// Only a bounded number of columns are ticketed ahead, so the generation pool
// /// focuses on the closest chunks first instead of spawning everything at once.
// fn ticket_pending_columns(
//     mut players: Query<(&mut PlayerColumnView, &InDimension, &Reposition)>,
//     mut dimensions: Query<&mut ChunkTicketsCommands>,
// ) {
//     /// How many un-sent columns may have outstanding tickets at a time.
//     const TICKET_AHEAD: usize = usize::MAX;
//
//     for (mut view, dim, rep) in &mut players {
//         let Ok(mut tickets) = dimensions.get_mut(dim.entity()) else {
//             continue;
//         };
//         let off = offset_sections(rep);
//         let view = &mut *view;
//
//         // Collect columns that need ticketing from the front of the queue.
//         let to_ticket: Vec<_> = view
//             .load_queue
//             .iter()
//             .filter(|col| view.desired_columns.contains(col))
//             .take(TICKET_AHEAD)
//             .filter(|col| !view.ticketed_columns.contains(col))
//             .copied()
//             .collect();
//
//         for col in to_ticket {
//             view.ticketed_columns.insert(col);
//             apply_forced_tickets(&mut tickets, col, off, true);
//         }
//     }
// }
//
// fn process_column_queues(
//     mut players: Query<(
//         Entity,
//         &mut PlayerColumnView,
//         &InDimension,
//         &Reposition,
//         &mut ServerSideConnection,
//     )>,
//     dims: Query<&ChunkIndex>,
//     chunks: Query<(&BlockPalette, &BiomePalette), With<ChunkLoaded>>,
// ) {
//     const MAX_COL_SENDS: usize = 64;
//
//     for (player, mut view, dim, rep, mut con) in &mut players {
//         let Ok(chunk_index) = dims.get(**dim) else {
//             continue;
//         };
//
//         let off = offset_sections(rep);
//
//         // Unloads first.
//         let mut sends = 0usize;
//         while sends < MAX_COL_SENDS {
//             let Some(col) = view.unload_queue.pop_front() else {
//                 break;
//             };
//             if view.sent_columns.remove(&col) {
//                 // unload_out.write(PlayerColumnUnloadRequest { player, column_pos: col });
//                 sends += 1;
//             }
//         }
//
//         // Loads / resends.
//         // Reuse a single buffer across all columns to avoid per-chunk allocation.
//         let mut data = Vec::with_capacity(8 * 1024);
//         sends = 0usize;
//         while sends < MAX_COL_SENDS {
//             let Some(column_pos) = view.load_queue.front().copied() else {
//                 break;
//             };
//
//             // Column may have left desired set while waiting in queue.
//             if !view.desired_columns.contains(&column_pos) {
//                 view.load_queue.pop_front();
//                 view.queued_columns.remove(&column_pos);
//                 continue;
//             }
//
//             // Gather 16 server chunk entities in client section order.
//             let mut ready = true;
//             data.clear();
//
//             for client_y in 0..CLIENT_COLUMN_SECTIONS {
//                 let server_y = client_y - off;
//                 let pos = ChunkPos::new(column_pos.x, server_y, column_pos.z);
//
//                 let Some(chunk_e) = chunk_index.get(pos) else {
//                     ready = false;
//                     break;
//                 };
//                 let Ok((blocks, biomes)) = chunks.get(chunk_e) else {
//                     ready = false;
//                     break;
//                 };
//                 blocks
//                     .non_air_block_count()
//                     .encode(&mut data)
//                     .expect("Failed to encode chunk block count");
//                 blocks
//                     .convert_network()
//                     .encode(&mut data)
//                     .expect("Failed to encode chunk block data");
//                 biomes
//                     .convert_network()
//                     .encode(&mut data)
//                     .expect("Failed to encode chunk biome data");
//             }
//
//             if !ready {
//                 break;
//             }
//
//             view.load_queue.pop_front();
//             view.queued_columns.remove(&column_pos);
//             view.sent_columns.insert(column_pos);
//             let pkt = ClientboundLevelChunkWithLight {
//                 pos: ColumnPos::new(
//                     rep.convert_chunk_x(column_pos.x),
//                     rep.convert_chunk_z(column_pos.z),
//                 ),
//                 chunk_data: ChunkData {
//                     data: data.as_slice(),
//                     ..Default::default()
//                 },
//                 light_data: LightData::default(),
//             };
//             con.write_packet(&pkt);
//             sends += 1;
//         }
//     }
// }
//
// pub fn update_reposition_from_transform(
//     cfg: Res<RepositionConfig>,
//     mut q: Query<(&Transform, &mut Reposition), Changed<Transform>>,
// ) {
//     for (tf, mut rep) in &mut q {
//         let y_blocks = tf.translation.y.floor() as i32;
//         rep.ensure_visible_y_window(y_blocks, cfg.min_y, cfg.max_y, cfg.step_y);
//     }
// }
