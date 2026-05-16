//! Cross-chunk distribute pass: drains `*Egress` wavefronts from source
//! chunks and pre-attenuates them onto the destination chunk's
//! `*Incoming` (or `*PendingEgress` on overflow).
//!
//! Face-direction contract:
//!
//! * BFS emits to `*Egress` with `face = direction-from-source-cell OUT of
//!   the source chunk` (source frame).
//! * Distribute pre-attenuates and writes to the destination's `*Incoming`
//!   with `face = direction-from-destination-cell IN from the source
//!   chunk` (destination frame — the opposite of the source's face).
//! * The neighbor-edge pull system reuses the destination-frame convention.
//!
//! Three-pass shape (mandated by Bevy's borrow checker — a single query
//! cannot hold `&mut *Egress` on a source and `&mut *Incoming` on a
//! destination simultaneously when both may resolve to the same entity at
//! adjacent ticks): Pass A drains `*Egress` into a `Local` staging buffer,
//! Pass B applies staged wavefronts to `*Incoming`, Pass C inserts the
//! `LightDirty` + `LightTicket` markers on each unique destination via a
//! `Local` dedup set.

use bevy_ecs::entity::EntityHashSet;
use bevy_ecs::prelude::*;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::components::{
    BlockEgress, BlockIncoming, BlockPendingEgress, LightDirty, NeedsFullReseed, SkyEgress,
    SkyIncoming, SkyPendingEgress, Wavefront,
};
use crate::converge::PENDING_EGRESS_CAP;
use crate::telemetry::{LIGHT_CROSS_DIM_VIOLATIONS_TOTAL, LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::chunk::ChunkPos;
use mcrs_engine::world::column::{
    ColumnPos, ColumnIndex, InColumn, ColumnChunks, ChunkLookup,
};
use mcrs_engine::world::dimension::InDimension;
use mcrs_engine::world::lighting::LightTicket;

/// Manhattan attenuation: face-adjacent (1), edge (2), corner (3). The
/// `max(1)` floor guarantees at least one step of attenuation even if a
/// caller passes `adjacency = 0`, matching the cross-chunk invariant that
/// emission across a chunk boundary always loses at least one level.
#[inline]
pub(crate) fn manhattan_preattenuate(level: u8, adjacency: u8) -> u8 {
    level.saturating_sub(adjacency.max(1))
}

/// Decode a packed face byte back into a `Direction`. Byte ordering matches
/// `Direction::index()` in `mcrs_core::voxel_shape`.
#[inline]
pub(crate) fn direction_from_index(byte: u8) -> Direction {
    match byte {
        0 => Direction::Down,
        1 => Direction::Up,
        2 => Direction::North,
        3 => Direction::South,
        4 => Direction::West,
        5 => Direction::East,
        _ => unreachable!("invalid face index {byte}"),
    }
}

/// Outcome of resolving the destination chunk for a wavefront.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolveOutcome {
    /// Destination chunk is loaded and addressable as a single entity.
    Loaded {
        dst_entity: Entity,
        dst_chunk_pos: ChunkPos,
        dst_column: Entity,
    },
    /// Destination column exists but the chunk at the target Y is not yet
    /// loaded. Wavefront must be parked on the source's `*PendingEgress`.
    Unloaded {
        dst_column: Entity,
        dst_chunk_pos: ChunkPos,
    },
    /// Destination is the per-column top/bottom padding row — drop silently.
    Padding,
    /// Destination Y is outside the column's range — drop silently.
    OutOfRange,
}

/// Resolve the destination of a wavefront. Up/Down route through the source
/// column's `ColumnChunks`; N/S/E/W route through the dimension's
/// `ColumnIndex` to find the neighbour column entity, then its
/// `ColumnChunks`. Returns `None` only when the column entity itself is
/// missing from `ColumnIndex` (e.g., source's column was despawned in a
/// concurrent tick), or when the source's column has no `ColumnChunks`.
pub(crate) fn resolve_neighbor_chunk(
    src_chunk_pos: ChunkPos,
    src_in_col: InColumn,
    src_in_dim: InDimension,
    face: Direction,
    column_indexes: &Query<&ColumnIndex>,
    chunk_indexes: &Query<&ColumnChunks>,
) -> Option<ResolveOutcome> {
    match face {
        Direction::Up | Direction::Down => {
            let dst_y = match face {
                Direction::Up => src_chunk_pos.y + 1,
                Direction::Down => src_chunk_pos.y - 1,
                _ => unreachable!(),
            };
            let chunk_index = chunk_indexes.get(src_in_col.0).ok()?;
            let lookup = chunk_index.lookup(dst_y);
            let dst_chunk_pos = ChunkPos::new(src_chunk_pos.x, dst_y, src_chunk_pos.z);
            Some(match lookup {
                ChunkLookup::Loaded(dst_entity) => ResolveOutcome::Loaded {
                    dst_entity,
                    dst_chunk_pos,
                    dst_column: src_in_col.0,
                },
                ChunkLookup::Unloaded => ResolveOutcome::Unloaded {
                    dst_column: src_in_col.0,
                    dst_chunk_pos,
                },
                ChunkLookup::BottomPadding | ChunkLookup::TopPadding => {
                    ResolveOutcome::Padding
                }
                ChunkLookup::OutOfRange => ResolveOutcome::OutOfRange,
            })
        }
        Direction::North | Direction::South | Direction::West | Direction::East => {
            let (dx, dz) = match face {
                Direction::East => (1, 0),
                Direction::West => (-1, 0),
                Direction::South => (0, 1),
                Direction::North => (0, -1),
                _ => unreachable!(),
            };
            let neighbour_col_pos =
                ColumnPos::new(src_chunk_pos.x + dx, src_chunk_pos.z + dz);
            let column_index = column_indexes.get(src_in_dim.0).ok()?;
            let slot = column_index.0.get(&neighbour_col_pos)?;
            let dst_column = slot.entity;
            let chunk_index = chunk_indexes.get(dst_column).ok()?;
            let lookup = chunk_index.lookup(src_chunk_pos.y);
            let dst_chunk_pos =
                ChunkPos::new(neighbour_col_pos.x, src_chunk_pos.y, neighbour_col_pos.z);
            Some(match lookup {
                ChunkLookup::Loaded(dst_entity) => ResolveOutcome::Loaded {
                    dst_entity,
                    dst_chunk_pos,
                    dst_column,
                },
                ChunkLookup::Unloaded => ResolveOutcome::Unloaded {
                    dst_column,
                    dst_chunk_pos,
                },
                ChunkLookup::BottomPadding | ChunkLookup::TopPadding => {
                    ResolveOutcome::Padding
                }
                ChunkLookup::OutOfRange => ResolveOutcome::OutOfRange,
            })
        }
    }
}

#[inline]
fn rate_limited_xdim_log(
    last_log: &mut Option<Instant>,
    src: Entity,
    dst: Entity,
    src_dim: Option<Entity>,
    dst_dim: Option<Entity>,
) {
    let now = Instant::now();
    let should_log = match *last_log {
        None => true,
        Some(prev) => now.duration_since(prev) >= Duration::from_secs(1),
    };
    if should_log {
        tracing::error!(
            ?src,
            ?dst,
            ?src_dim,
            ?dst_dim,
            "cross-dim wavefront route attempted; dropping"
        );
        *last_log = Some(now);
    }
}

pub fn distribute_decrease(
    block_sources: Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut BlockEgress)>,
    sky_sources: Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut SkyEgress)>,
    block_incoming: Query<&mut BlockIncoming>,
    sky_incoming: Query<&mut SkyIncoming>,
    block_pending: Query<&mut BlockPendingEgress>,
    sky_pending: Query<&mut SkyPendingEgress>,
    in_dimensions: Query<&InDimension>,
    chunk_indexes: Query<&ColumnChunks>,
    column_indexes: Query<&ColumnIndex>,
    block_stage: Local<Vec<(Entity, Wavefront)>>,
    sky_stage: Local<Vec<(Entity, Wavefront)>>,
    dirty_dedup: Local<EntityHashSet>,
    last_xdim_log: Local<Option<Instant>>,
    commands: Commands,
) {
    #[cfg(feature = "lighting-trace")]
    let block_egress_count = block_sources.iter().count();
    #[cfg(feature = "lighting-trace")]
    let sky_egress_count = sky_sources.iter().count();
    #[cfg(feature = "lighting-trace")]
    let _span = tracing::info_span!("distribute_decrease", block_egress_count = block_egress_count, sky_egress_count = sky_egress_count).entered();
    distribute_inner(
        block_sources,
        sky_sources,
        block_incoming,
        sky_incoming,
        block_pending,
        sky_pending,
        in_dimensions,
        chunk_indexes,
        column_indexes,
        block_stage,
        sky_stage,
        dirty_dedup,
        last_xdim_log,
        commands,
    );
}

pub fn distribute_increase(
    block_sources: Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut BlockEgress)>,
    sky_sources: Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut SkyEgress)>,
    block_incoming: Query<&mut BlockIncoming>,
    sky_incoming: Query<&mut SkyIncoming>,
    block_pending: Query<&mut BlockPendingEgress>,
    sky_pending: Query<&mut SkyPendingEgress>,
    in_dimensions: Query<&InDimension>,
    chunk_indexes: Query<&ColumnChunks>,
    column_indexes: Query<&ColumnIndex>,
    block_stage: Local<Vec<(Entity, Wavefront)>>,
    sky_stage: Local<Vec<(Entity, Wavefront)>>,
    dirty_dedup: Local<EntityHashSet>,
    last_xdim_log: Local<Option<Instant>>,
    commands: Commands,
) {
    #[cfg(feature = "lighting-trace")]
    let block_egress_count = block_sources.iter().count();
    #[cfg(feature = "lighting-trace")]
    let sky_egress_count = sky_sources.iter().count();
    #[cfg(feature = "lighting-trace")]
    let _span = tracing::info_span!("distribute_increase", block_egress_count = block_egress_count, sky_egress_count = sky_egress_count).entered();
    // `distribute_increase` and `distribute_decrease` route wavefronts the
    // same way; the increase-versus-decrease distinction lives entirely in
    // the intra-chunk BFS that produced the wavefront. The two systems
    // exist separately so they can be scheduled at distinct points in
    // `LightConvergeSchedule` even though they share the same body.
    distribute_inner(
        block_sources,
        sky_sources,
        block_incoming,
        sky_incoming,
        block_pending,
        sky_pending,
        in_dimensions,
        chunk_indexes,
        column_indexes,
        block_stage,
        sky_stage,
        dirty_dedup,
        last_xdim_log,
        commands,
    );
}

fn distribute_inner(
    mut block_sources: Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut BlockEgress)>,
    mut sky_sources: Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut SkyEgress)>,
    mut block_incoming: Query<&mut BlockIncoming>,
    mut sky_incoming: Query<&mut SkyIncoming>,
    mut block_pending: Query<&mut BlockPendingEgress>,
    mut sky_pending: Query<&mut SkyPendingEgress>,
    in_dimensions: Query<&InDimension>,
    chunk_indexes: Query<&ColumnChunks>,
    column_indexes: Query<&ColumnIndex>,
    mut block_stage: Local<Vec<(Entity, Wavefront)>>,
    mut sky_stage: Local<Vec<(Entity, Wavefront)>>,
    mut dirty_dedup: Local<EntityHashSet>,
    mut last_xdim_log: Local<Option<Instant>>,
    mut commands: Commands,
) {
    block_stage.clear();
    sky_stage.clear();
    dirty_dedup.clear();

    drain_block_egress(
        &mut block_sources,
        &mut block_pending,
        &in_dimensions,
        &chunk_indexes,
        &column_indexes,
        &mut block_stage,
        &mut last_xdim_log,
        &mut commands,
    );

    drain_sky_egress(
        &mut sky_sources,
        &mut sky_pending,
        &in_dimensions,
        &chunk_indexes,
        &column_indexes,
        &mut sky_stage,
        &mut last_xdim_log,
        &mut commands,
    );

    for (dst_entity, wavefront) in block_stage.drain(..) {
        if let Ok(mut incoming) = block_incoming.get_mut(dst_entity) {
            incoming.0.push(wavefront);
            dirty_dedup.insert(dst_entity);
        }
    }

    for (dst_entity, wavefront) in sky_stage.drain(..) {
        if let Ok(mut incoming) = sky_incoming.get_mut(dst_entity) {
            incoming.0.push(wavefront);
            dirty_dedup.insert(dst_entity);
        }
    }

    for dst in dirty_dedup.drain() {
        commands.entity(dst).insert(LightDirty);
        commands.entity(dst).insert(LightTicket);
    }
}

fn drain_block_egress(
    sources: &mut Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut BlockEgress)>,
    pending: &mut Query<&mut BlockPendingEgress>,
    in_dimensions: &Query<&InDimension>,
    chunk_indexes: &Query<&ColumnChunks>,
    column_indexes: &Query<&ColumnIndex>,
    stage: &mut Vec<(Entity, Wavefront)>,
    last_xdim_log: &mut Option<Instant>,
    commands: &mut Commands,
) {
    for (src_entity, chunk_pos, in_dim, in_col, mut egress) in sources.iter_mut() {
        if egress.0.is_empty() {
            continue;
        }
        commands.entity(src_entity).insert(LightTicket);

        let src_dim = in_dim.0;
        // Pre-resolve all six neighbour faces once per source instead of once
        // per wavefront; see comment in `drain_sky_egress`.
        let resolved_faces: [Option<ResolveOutcome>; 6] = [
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::Down,  column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::Up,    column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::North, column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::South, column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::West,  column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::East,  column_indexes, chunk_indexes),
        ];

        let drained = std::mem::take(&mut egress.0);
        for wavefront in drained {
            let face = direction_from_index(wavefront.face());
            // The intra-chunk BFS emits face-adjacent wavefronts only
            // (adjacency = 1). Edge (2) / corner (3) paths live in
            // `manhattan_preattenuate` for completeness and are exercised
            // by dedicated unit tests; the active BFS does not yet emit
            // diagonal wavefronts.
            let pre_attenuated_level = manhattan_preattenuate(wavefront.level(), 1);

            let outcome = resolved_faces[face.index()];

            match outcome {
                Some(ResolveOutcome::Loaded { dst_entity, .. }) => {
                    let dst_dim_opt = in_dimensions.get(dst_entity).map(|d| d.0).ok();
                    let src_dim_opt = Some(src_dim);
                    debug_assert_eq!(
                        dst_dim_opt, src_dim_opt,
                        "cross-dim wavefront route attempted"
                    );
                    if dst_dim_opt != src_dim_opt {
                        LIGHT_CROSS_DIM_VIOLATIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
                        rate_limited_xdim_log(
                            last_xdim_log,
                            src_entity,
                            dst_entity,
                            src_dim_opt,
                            dst_dim_opt,
                        );
                        continue;
                    }
                    let dest_face = face.opposite().index() as u8;
                    stage.push((
                        dst_entity,
                        Wavefront::new(
                            dest_face,
                            wavefront.cell_x(),
                            wavefront.cell_z(),
                            pre_attenuated_level,
                        ),
                    ));
                }
                Some(ResolveOutcome::Unloaded { dst_column, .. }) => {
                    if let Ok(mut pend) = pending.get_mut(src_entity) {
                        if pend.0.len() >= PENDING_EGRESS_CAP {
                            LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL
                                .fetch_add(1, Ordering::Relaxed);
                            tracing::warn!(
                                target: "mcrs_lighting::needs_full_reseed",
                                src = ?src_entity,
                                dst_column = ?dst_column,
                                src_chunk_x = chunk_pos.x,
                                src_chunk_y = chunk_pos.y,
                                src_chunk_z = chunk_pos.z,
                                kind = "block_egress_overflow",
                                pending_cap = PENDING_EGRESS_CAP,
                                "Block-egress pending overflow — inserting NeedsFullReseed on destination column."
                            );
                            commands.entity(dst_column).insert(NeedsFullReseed);
                        } else {
                            pend.0.push(wavefront);
                        }
                    }
                }
                Some(ResolveOutcome::Padding)
                | Some(ResolveOutcome::OutOfRange)
                | None => {}
            }
        }
    }
}

fn drain_sky_egress(
    sources: &mut Query<(Entity, &ChunkPos, &InDimension, &InColumn, &mut SkyEgress)>,
    pending: &mut Query<&mut SkyPendingEgress>,
    in_dimensions: &Query<&InDimension>,
    chunk_indexes: &Query<&ColumnChunks>,
    column_indexes: &Query<&ColumnIndex>,
    stage: &mut Vec<(Entity, Wavefront)>,
    last_xdim_log: &mut Option<Instant>,
    commands: &mut Commands,
) {
    for (src_entity, chunk_pos, in_dim, in_col, mut egress) in sources.iter_mut() {
        if egress.0.is_empty() {
            continue;
        }
        commands.entity(src_entity).insert(LightTicket);

        let src_dim = in_dim.0;
        // The column-walker fast path emits 1280 wavefronts (5 faces × 256
        // cells) per source per iteration. Calling `resolve_neighbor_chunk`
        // for each one walks `ColumnChunks` / `ColumnIndex` hash lookups
        // afresh, which dominates the sub-schedule's wall clock at chunk-load
        // time. Resolve each of the six faces once up front and index into
        // the array per wavefront. Same destination semantics, roughly 250x
        // fewer hash lookups per source.
        let resolved_faces: [Option<ResolveOutcome>; 6] = [
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::Down,  column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::Up,    column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::North, column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::South, column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::West,  column_indexes, chunk_indexes),
            resolve_neighbor_chunk(*chunk_pos, *in_col, *in_dim, Direction::East,  column_indexes, chunk_indexes),
        ];

        let drained = std::mem::take(&mut egress.0);
        for wavefront in drained {
            let face = direction_from_index(wavefront.face());
            // Sky-light entering a destination cell via its Up face (i.e. the
            // source pushed it through its Down face) propagates without
            // attenuation when the destination cell carries
            // `PROPAGATES_SKYLIGHT_DOWN`. distribute lacks access to the
            // destination palette, but for sky-light the only Down-face
            // wavefronts come from cells that themselves passed the
            // `PROPAGATES_SKYLIGHT_DOWN` check in the source BFS or from the
            // column-walker fast path (which only fires on all-air chunks).
            // Skip the cross-boundary -1 in that case so the receiving
            // chunk's column-walker condition (`level == 15`) keeps
            // triggering down the column. The destination chunk's BFS
            // re-applies opacity attenuation per cell, so opaque cells in the
            // destination still cap their level via the `dst_flags` /
            // `opacity` check in `propagate_increase_sky`.
            let pre_attenuated_level = if face == Direction::Down {
                wavefront.level()
            } else {
                manhattan_preattenuate(wavefront.level(), 1)
            };

            let outcome = resolved_faces[face.index()];

            match outcome {
                Some(ResolveOutcome::Loaded { dst_entity, .. }) => {
                    let dst_dim_opt = in_dimensions.get(dst_entity).map(|d| d.0).ok();
                    let src_dim_opt = Some(src_dim);
                    debug_assert_eq!(
                        dst_dim_opt, src_dim_opt,
                        "cross-dim wavefront route attempted"
                    );
                    if dst_dim_opt != src_dim_opt {
                        LIGHT_CROSS_DIM_VIOLATIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
                        rate_limited_xdim_log(
                            last_xdim_log,
                            src_entity,
                            dst_entity,
                            src_dim_opt,
                            dst_dim_opt,
                        );
                        continue;
                    }
                    let dest_face = face.opposite().index() as u8;
                    stage.push((
                        dst_entity,
                        Wavefront::new(
                            dest_face,
                            wavefront.cell_x(),
                            wavefront.cell_z(),
                            pre_attenuated_level,
                        ),
                    ));
                }
                Some(ResolveOutcome::Unloaded { dst_column, .. }) => {
                    if let Ok(mut pend) = pending.get_mut(src_entity) {
                        if pend.0.len() >= PENDING_EGRESS_CAP {
                            LIGHT_PENDING_EGRESS_OVERFLOW_TOTAL
                                .fetch_add(1, Ordering::Relaxed);
                            tracing::warn!(
                                target: "mcrs_lighting::needs_full_reseed",
                                src = ?src_entity,
                                dst_column = ?dst_column,
                                src_chunk_x = chunk_pos.x,
                                src_chunk_y = chunk_pos.y,
                                src_chunk_z = chunk_pos.z,
                                kind = "sky_egress_overflow",
                                pending_cap = PENDING_EGRESS_CAP,
                                "Sky-egress pending overflow — inserting NeedsFullReseed on destination column. \
                                 This will re-mark every loaded chunk in the destination column for initial-light seeding, \
                                 which can re-fire seed_initial_light against a heightmap that may still be at sentinel."
                            );
                            commands.entity(dst_column).insert(NeedsFullReseed);
                        } else {
                            pend.0.push(wavefront);
                        }
                    }
                }
                Some(ResolveOutcome::Padding)
                | Some(ResolveOutcome::OutOfRange)
                | None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, Update};
    use mcrs_engine::world::column::{ColumnPos, ColumnSlot};
    use smallvec::SmallVec;

    use crate::telemetry::TELEMETRY_TEST_LOCK;

    fn build_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, distribute_decrease);
        app
    }

    fn spawn_dimension(app: &mut App) -> Entity {
        app.world_mut().spawn(ColumnIndex::default()).id()
    }

    fn spawn_column(app: &mut App, min_chunk_y: i32, slot_count: usize) -> Entity {
        app.world_mut()
            .spawn(ColumnChunks::new(min_chunk_y, slot_count))
            .id()
    }

    fn register_column(app: &mut App, dim: Entity, col_pos: ColumnPos, col_entity: Entity) {
        let mut ci = app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("dimension has ColumnIndex");
        ci.0.insert(
            col_pos,
            ColumnSlot {
                entity: col_entity,
                section_count: 1,
            },
        );
    }

    fn spawn_block_chunk(
        app: &mut App,
        chunk_pos: ChunkPos,
        column: Entity,
        dim: Entity,
        egress: SmallVec<[Wavefront; 8]>,
    ) -> Entity {
        let chunk = app
            .world_mut()
            .spawn((
                chunk_pos,
                InDimension(dim),
                InColumn(column),
                BlockEgress(egress),
                BlockIncoming::default(),
                BlockPendingEgress::default(),
            ))
            .id();
        if let Some(mut si) = app.world_mut().get_mut::<ColumnChunks>(column) {
            si.set_loaded(chunk_pos.y, chunk);
        }
        chunk
    }

    /// (dim, col_a, col_b, chunk_a, chunk_b) — two columns at (0,0) and (1,0)
    /// each with one chunk at chunk-Y 0. Both chunks live in the same
    /// dimension.
    fn make_two_column_world(
        app: &mut App,
        egress_a: SmallVec<[Wavefront; 8]>,
    ) -> (Entity, Entity, Entity, Entity, Entity) {
        let dim = spawn_dimension(app);
        let col_a = spawn_column(app, 0, 1);
        let col_b = spawn_column(app, 0, 1);
        register_column(app, dim, ColumnPos::new(0, 0), col_a);
        register_column(app, dim, ColumnPos::new(1, 0), col_b);
        let chunk_a = spawn_block_chunk(app, ChunkPos::new(0, 0, 0), col_a, dim, egress_a);
        let chunk_b =
            spawn_block_chunk(app, ChunkPos::new(1, 0, 0), col_b, dim, SmallVec::new());
        (dim, col_a, col_b, chunk_a, chunk_b)
    }

    #[test]
    fn manhattan_preattenuate_face_edge_corner() {
        assert_eq!(manhattan_preattenuate(15, 1), 14);
        assert_eq!(manhattan_preattenuate(15, 2), 13);
        assert_eq!(manhattan_preattenuate(15, 3), 12);
        assert_eq!(manhattan_preattenuate(3, 5), 0);
    }

    #[test]
    fn distribute_decrease_routes_face_adjacent() {
        let mut app = build_app();
        let east = Direction::East.index() as u8;
        let mut egress = SmallVec::new();
        egress.push(Wavefront::new(east, 4, 7, 8));
        let (_dim, _col_a, _col_b, chunk_a, chunk_b) = make_two_column_world(&mut app, egress);

        app.update();

        let incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("chunk_b has BlockIncoming");
        assert_eq!(incoming.0.len(), 1, "exactly one wavefront delivered");
        let w = incoming.0[0];
        assert_eq!(w.face(), Direction::West.index() as u8);
        assert_eq!(w.cell_x(), 4);
        assert_eq!(w.cell_z(), 7);
        assert_eq!(w.level(), 7, "Manhattan-1 attenuated from 8 to 7");

        let src_egress = app
            .world()
            .get::<BlockEgress>(chunk_a)
            .expect("chunk_a");
        assert!(src_egress.0.is_empty(), "source egress drained");

        assert!(app.world().get::<LightDirty>(chunk_b).is_some());
        assert!(app.world().get::<LightTicket>(chunk_b).is_some());
    }

    #[test]
    fn distribute_decrease_routes_edge_adjacent() {
        // Edge (adjacency = 2) attenuation path. The production BFS does not
        // currently emit edge wavefronts; this test exercises the dead-code
        // pre-attenuation arm per C-05.
        assert_eq!(manhattan_preattenuate(15, 2), 13);
    }

    #[test]
    fn distribute_decrease_routes_corner_adjacent() {
        assert_eq!(manhattan_preattenuate(15, 3), 12);
    }

    #[test]
    fn distribute_pending_egress_overflow_inserts_needs_full_reseed() {
        let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = build_app();
        let dim = spawn_dimension(&mut app);
        let col_a = spawn_column(&mut app, 0, 1);
        let col_b = spawn_column(&mut app, 0, 1);
        register_column(&mut app, dim, ColumnPos::new(0, 0), col_a);
        register_column(&mut app, dim, ColumnPos::new(1, 0), col_b);
        // col_b's chunk slot stays None — destination resolves to Unloaded.

        let east = Direction::East.index() as u8;
        let mut egress = SmallVec::new();
        egress.push(Wavefront::new(east, 0, 0, 10));
        let mut prefill = SmallVec::new();
        for i in 0..PENDING_EGRESS_CAP {
            prefill.push(Wavefront::new(east, (i % 16) as u8, ((i / 16) % 16) as u8, 5));
        }
        let chunk_a = app
            .world_mut()
            .spawn((
                ChunkPos::new(0, 0, 0),
                InDimension(dim),
                InColumn(col_a),
                BlockEgress(egress),
                BlockIncoming::default(),
                BlockPendingEgress(prefill),
            ))
            .id();
        if let Some(mut si) = app.world_mut().get_mut::<ColumnChunks>(col_a) {
            si.set_loaded(0, chunk_a);
        }

        let snap_before = crate::telemetry::snapshot();
        app.update();
        let snap_after = crate::telemetry::snapshot();

        assert_eq!(
            snap_after.overflow - snap_before.overflow,
            1,
            "overflow counter incremented exactly once"
        );
        let pend = app
            .world()
            .get::<BlockPendingEgress>(chunk_a)
            .expect("source pending");
        assert_eq!(
            pend.0.len(),
            PENDING_EGRESS_CAP,
            "pending stays at cap; new wavefront dropped"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(col_b).is_some(),
            "destination column got NeedsFullReseed"
        );
    }

    #[test]
    fn distribute_pre_attenuates_face_adjacent() {
        let mut app = build_app();
        let east = Direction::East.index() as u8;
        let mut egress = SmallVec::new();
        egress.push(Wavefront::new(east, 0, 0, 10));
        let (_dim, _col_a, _col_b, _chunk_a, chunk_b) =
            make_two_column_world(&mut app, egress);

        app.update();

        let incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("chunk_b has BlockIncoming");
        assert_eq!(incoming.0[0].level(), 9, "10 - 1 = 9 on face-adjacent route");
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "cross-dim wavefront route attempted")]
    fn distribute_panics_on_cross_dim_in_debug_build() {
        let mut app = build_app();
        let dim_a = spawn_dimension(&mut app);
        let dim_b = spawn_dimension(&mut app);
        let col_a = spawn_column(&mut app, 0, 1);
        let col_b = spawn_column(&mut app, 0, 1);
        register_column(&mut app, dim_a, ColumnPos::new(0, 0), col_a);
        register_column(&mut app, dim_a, ColumnPos::new(1, 0), col_b);

        let east = Direction::East.index() as u8;
        let mut egress = SmallVec::new();
        egress.push(Wavefront::new(east, 0, 0, 10));

        let _chunk_b = spawn_block_chunk(
            &mut app,
            ChunkPos::new(1, 0, 0),
            col_b,
            dim_b,
            SmallVec::new(),
        );

        let _chunk_a = spawn_block_chunk(
            &mut app,
            ChunkPos::new(0, 0, 0),
            col_a,
            dim_a,
            egress,
        );

        app.update();
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn distribute_increments_cross_dim_counter_in_release() {
        let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut app = build_app();
        let dim_a = spawn_dimension(&mut app);
        let dim_b = spawn_dimension(&mut app);
        let col_a = spawn_column(&mut app, 0, 1);
        let col_b = spawn_column(&mut app, 0, 1);
        register_column(&mut app, dim_a, ColumnPos::new(0, 0), col_a);
        register_column(&mut app, dim_a, ColumnPos::new(1, 0), col_b);

        let east = Direction::East.index() as u8;
        let mut egress = SmallVec::new();
        egress.push(Wavefront::new(east, 0, 0, 10));

        let chunk_b = spawn_block_chunk(
            &mut app,
            ChunkPos::new(1, 0, 0),
            col_b,
            dim_b,
            SmallVec::new(),
        );

        let _chunk_a = spawn_block_chunk(
            &mut app,
            ChunkPos::new(0, 0, 0),
            col_a,
            dim_a,
            egress,
        );

        let before = crate::telemetry::snapshot();
        app.update();
        let after = crate::telemetry::snapshot();

        assert_eq!(after.cross_dim - before.cross_dim, 1);
        let incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("chunk_b has BlockIncoming");
        assert!(
            incoming.0.is_empty(),
            "cross-dim wavefront dropped, not written"
        );
    }

    #[test]
    fn distribute_inserts_light_ticket_on_source_with_egress() {
        let mut app = build_app();
        let east = Direction::East.index() as u8;
        let mut egress = SmallVec::new();
        egress.push(Wavefront::new(east, 0, 0, 8));
        let (_dim, _col_a, _col_b, chunk_a, _chunk_b) =
            make_two_column_world(&mut app, egress);

        app.update();

        assert!(
            app.world().get::<LightTicket>(chunk_a).is_some(),
            "source with non-empty egress got LightTicket"
        );
    }

    #[test]
    fn distribute_inserts_light_ticket_on_destination_once() {
        let mut app = build_app();
        let east = Direction::East.index() as u8;
        // 8 wavefronts all targeting the same destination — dedup must
        // collapse to one LightDirty + LightTicket insert.
        let mut egress = SmallVec::new();
        for cz in 0..8u8 {
            egress.push(Wavefront::new(east, 0, cz, 8));
        }
        let (_dim, _col_a, _col_b, _chunk_a, chunk_b) =
            make_two_column_world(&mut app, egress);

        app.update();

        assert!(app.world().get::<LightDirty>(chunk_b).is_some());
        assert!(app.world().get::<LightTicket>(chunk_b).is_some());
        let incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("chunk_b incoming");
        assert_eq!(incoming.0.len(), 8, "all 8 wavefronts delivered");
    }

    #[test]
    fn distribute_drops_wavefronts_to_padding() {
        // Source at chunk-Y 0 in a column whose ColumnChunks only covers y=0.
        // A Down-face wavefront lands on BottomPadding (relative y=-1) which
        // must be dropped silently — no LightDirty/LightTicket on the source,
        // no pending egress, no incoming written anywhere.
        let mut app = build_app();
        let dim = spawn_dimension(&mut app);
        let col_a = spawn_column(&mut app, 0, 1);
        register_column(&mut app, dim, ColumnPos::new(0, 0), col_a);

        let down = Direction::Down.index() as u8;
        let mut egress = SmallVec::new();
        egress.push(Wavefront::new(down, 5, 5, 8));

        let chunk_a =
            spawn_block_chunk(&mut app, ChunkPos::new(0, 0, 0), col_a, dim, egress);

        app.update();

        let src_egress = app
            .world()
            .get::<BlockEgress>(chunk_a)
            .expect("chunk_a");
        assert!(src_egress.0.is_empty(), "source egress drained");
        let pend = app
            .world()
            .get::<BlockPendingEgress>(chunk_a)
            .expect("chunk_a pending");
        assert!(pend.0.is_empty(), "padding drop does not enter pending");
        // No NeedsFullReseed insertion (which the overflow path would emit).
        assert!(
            app.world().get::<NeedsFullReseed>(col_a).is_none(),
            "padding drop must not insert NeedsFullReseed"
        );
    }
}
