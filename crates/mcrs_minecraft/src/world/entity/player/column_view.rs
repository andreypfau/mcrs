use std::collections::VecDeque;

use crate::world::palette::{BiomePalette, BlockPalette};
use bevy_app::{App, FixedUpdate, Plugin, PreUpdate};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Added, Changed, Component, ContainsEntity, Message, On, Query, With};
use bevy_ecs::system::{Commands, Res};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::chunk_view::{
    ChunkTrackingView, ChunkTrackingViewUpdateEvent, ChunkViewPlugin, PlayerChunkObserver,
};
use mcrs_engine::entity::player::reposition::{Reposition, RepositionConfig};
use mcrs_engine::world::chunk::ticket::{ChunkTicketsCommands, Ticket, TicketKind};
use mcrs_engine::world::chunk::{ChunkIndex, ChunkLoaded, ChunkPos, ChunkStatus};
use mcrs_engine::world::dimension::InDimension;
use mcrs_network::ServerSideConnection;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundLevelChunkWithLight, ClientboundSetChunkCacheCenter,
};
use mcrs_protocol::{ChunkColumnPos, ChunkData, Encode, LightData, VarInt, WritePacket};
use rustc_hash::FxHashSet;
use tracing::{debug, trace};

/// Vanilla client window height: 256 blocks == 16 chunk sections.
const CLIENT_COLUMN_SECTIONS: i32 = 16;

pub struct ColumnViewPlugin;

impl Plugin for ColumnViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ChunkViewPlugin);

        // Initialize per-player column state.
        app.add_systems(PreUpdate, add_player_column_view);

        // React to ChunkTrackingView changes (xz-distance changes, movement).
        app.add_observer(on_view_update);

        // When vertical reposition offset changes, re-map forced tickets and re-send active columns.
        // app.add_systems(Update, handle_reposition_changed);

        // Drain column queues and produce load/unload requests.
        app.add_systems(FixedUpdate, process_column_queues);
    }
}

#[derive(Debug, Message)]
pub struct PlayerChunkColumnLoadRequest {
    pub player: Entity,
    pub column_pos: ChunkColumnPos,
    /// Server chunk entities in **client section order** (index 0..15 == client Y sections).
    pub sections: Vec<Entity>,
}

#[derive(Debug, Message)]
pub struct PlayerChunkColumnUnloadRequest {
    pub player: Entity,
    pub column_pos: ChunkColumnPos,
}

#[derive(Component, Default)]
pub struct PlayerColumnView {
    /// Columns the player should currently have (xz only).
    desired_columns: FxHashSet<ChunkColumnPos>,

    /// Columns that have already been sent at least once (xz only).
    sent_columns: FxHashSet<ChunkColumnPos>,

    /// Prevent duplicate enqueues.
    queued_columns: FxHashSet<ChunkColumnPos>,

    /// Columns pending (re)send.
    load_queue: VecDeque<ChunkColumnPos>,

    /// Columns pending unload.
    unload_queue: VecDeque<ChunkColumnPos>,

    /// Last applied vertical offset, in chunk-sections (blocks >> 4).
    last_offset_sections: i32,
}

fn add_player_column_view(
    players: Query<Entity, Added<PlayerChunkObserver>>,
    mut commands: Commands,
) {
    for player in &players {
        commands.entity(player).insert(PlayerColumnView::default());
    }
}

fn columns_for_view(view: &ChunkTrackingView, out: &mut FxHashSet<ChunkColumnPos>) {
    out.clear();
    let d = view.distance as i32;
    let cx = view.center.x;
    let cz = view.center.z;
    for x in (cx - d)..=(cx + d) {
        for z in (cz - d)..=(cz + d) {
            out.insert(ChunkColumnPos::new(x, z));
        }
    }
}

#[inline]
fn offset_sections(rep: &Reposition) -> i32 {
    let bits = mcrs_engine::world::chunk::BLOCKS::BITS as i32;
    rep.offset_y_blocks() >> bits
}

fn apply_forced_tickets(
    tickets: &mut ChunkTicketsCommands,
    col: ChunkColumnPos,
    off_sections: i32,
    add: bool,
) {
    for client_y in 0..CLIENT_COLUMN_SECTIONS {
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
    mut q: Query<(
        &mut ServerSideConnection,
        &Reposition,
        &InDimension,
        &mut PlayerColumnView,
        &mut PlayerChunkObserver,
    )>,
    mut dimensions: Query<&mut ChunkTicketsCommands>,
) {
    let Ok((mut con, rep, dim, mut col_view, mut observer)) = q.get_mut(event.player) else {
        return;
    };
    let Ok(mut tickets) = dimensions.get_mut(dim.entity()) else {
        return;
    };
    trace!(
        "Player {:?} chunk view updated: old={:?} new={:?}",
        event.player, event.old_view, event.new_view
    );
    let col_view = &mut *col_view;

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

    let new_off = offset_sections(rep);
    if col_view.desired_columns.is_empty() {
        // First time init for this player: initialize offset snapshot.
        col_view.last_offset_sections = new_off;
    }

    // Compute new desired columns set.
    let mut new_cols = FxHashSet::default();
    columns_for_view(&event.new_view, &mut new_cols);

    // Removed columns.
    for col in &col_view.desired_columns {
        if !new_cols.contains(col) {
            apply_forced_tickets(&mut tickets, *col, new_off, false);
            if col_view.sent_columns.contains(&col) {
                col_view.unload_queue.push_back(*col);
            }
            col_view.sent_columns.remove(&col);
            col_view.queued_columns.remove(&col);
        }
    }

    // Added columns.
    let center = ChunkColumnPos::from(event.new_view.center);
    let mut load_queue = Vec::with_capacity(new_cols.len());
    for col in new_cols.iter() {
        if !col_view.desired_columns.contains(col) {
            apply_forced_tickets(&mut tickets, *col, new_off, true);
            if col_view.queued_columns.insert(*col) {
                load_queue.push(*col);
            }
        }
    }
    load_queue.sort_unstable_by_key(|col| col.distance_squared(center));
    col_view.load_queue.extend(load_queue);

    col_view.desired_columns = new_cols;
}

/// When `Reposition` changes (vertical window shifts), update forced tickets for all active columns and re-send them.
fn handle_reposition_changed(
    mut players: Query<
        (
            &Reposition,
            &InDimension,
            &mut PlayerColumnView,
            &mut PlayerChunkObserver,
        ),
        Changed<Reposition>,
    >,
    mut dimensions: Query<&mut ChunkTicketsCommands>,
) {
    for (rep, dim, mut view, mut observer) in &mut players {
        let Ok(mut tickets) = dimensions.get_mut(dim.entity()) else {
            continue;
        };
        let view = &mut *view;
        let new_off = offset_sections(rep);
        let old_off = view.last_offset_sections;

        if new_off == old_off {
            continue;
        }

        // Remap forced tickets for every currently desired column.
        for col in (&view.desired_columns).iter() {
            // Remove old mapping.
            apply_forced_tickets(&mut tickets, *col, old_off, false);
            // Add new mapping.
            apply_forced_tickets(&mut tickets, *col, new_off, true);

            // Re-send column to client (overwrites sections in-place).
            if view.queued_columns.insert(*col) {
                view.load_queue.push_back(*col);
            }
        }

        view.last_offset_sections = new_off;
    }
}

fn process_column_queues(
    mut players: Query<(
        Entity,
        &mut PlayerColumnView,
        &InDimension,
        &Reposition,
        &mut ServerSideConnection,
    )>,
    dims: Query<&ChunkIndex>,
    chunks: Query<(&BlockPalette, &BiomePalette), With<ChunkLoaded>>,
) {
    const MAX_COL_SENDS: usize = 64;

    for (player, mut view, dim, rep, mut con) in &mut players {
        let Ok(chunk_index) = dims.get(**dim) else {
            continue;
        };

        let off = offset_sections(rep);

        // Unloads first.
        let mut sends = 0usize;
        while sends < MAX_COL_SENDS {
            let Some(col) = view.unload_queue.pop_front() else {
                break;
            };
            if view.sent_columns.remove(&col) {
                // unload_out.write(PlayerChunkColumnUnloadRequest { player, column_pos: col });
                sends += 1;
            }
        }

        // Loads / resends.
        // Reuse a single buffer across all columns to avoid per-chunk allocation.
        let mut data = Vec::with_capacity(8 * 1024);
        sends = 0usize;
        while sends < MAX_COL_SENDS {
            let Some(column_pos) = view.load_queue.front().copied() else {
                break;
            };

            // Column may have left desired set while waiting in queue.
            if !view.desired_columns.contains(&column_pos) {
                view.load_queue.pop_front();
                view.queued_columns.remove(&column_pos);
                continue;
            }

            // Gather 16 server chunk entities in client section order.
            let mut ready = true;
            data.clear();

            for client_y in 0..CLIENT_COLUMN_SECTIONS {
                let server_y = client_y - off;
                let pos = ChunkPos::new(column_pos.x, server_y, column_pos.z);

                let Some(chunk_e) = chunk_index.get(pos) else {
                    ready = false;
                    break;
                };
                let Ok((blocks, biomes)) = chunks.get(chunk_e) else {
                    ready = false;
                    break;
                };
                blocks
                    .non_air_block_count()
                    .encode(&mut data)
                    .expect("Failed to encode chunk block count");
                blocks
                    .convert_network()
                    .encode(&mut data)
                    .expect("Failed to encode chunk block data");
                biomes
                    .convert_network()
                    .encode(&mut data)
                    .expect("Failed to encode chunk biome data");
            }

            if !ready {
                break;
            }

            view.load_queue.pop_front();
            view.queued_columns.remove(&column_pos);
            view.sent_columns.insert(column_pos);
            let pkt = ClientboundLevelChunkWithLight {
                pos: ChunkColumnPos::new(
                    rep.convert_chunk_x(column_pos.x),
                    rep.convert_chunk_z(column_pos.z),
                ),
                chunk_data: ChunkData {
                    data: data.as_slice(),
                    ..Default::default()
                },
                light_data: LightData::default(),
            };
            con.write_packet(&pkt);
            sends += 1;
        }
    }
}

pub fn update_reposition_from_transform(
    cfg: Res<RepositionConfig>,
    mut q: Query<(&Transform, &mut Reposition), Changed<Transform>>,
) {
    for (tf, mut rep) in &mut q {
        let y_blocks = tf.translation.y.floor() as i32;
        rep.ensure_visible_y_window(y_blocks, cfg.min_y, cfg.max_y, cfg.step_y);
    }
}
