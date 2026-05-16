//! Consumes `MessageReader<BlockPlaced>`, derives the chunk's intra-cell
//! coord via `rem_euclid(16)` on i32, looks up old/new emission via
//! `Res<BlockLightTable>`, and pushes a decrease and/or increase seed into
//! the chunk's `BlockLightWorkspace` queues per the emission-diff rule:
//! `old_emission > new_emission` → decrease seed at `old_emission`,
//! `new_emission > 0` → increase seed at `new_emission`. `LightDirty` is
//! inserted via `Commands::entity(placed.chunk).insert(LightDirty)` when at
//! least one seed was pushed.
//!
//! Dampening-only changes (`old_emission == new_emission &&
//! old_dampening != new_dampening`) emit a `tracing::warn!` and skip; the
//! cell will desync until cross-chunk distribute lands. Missing
//! `BlockLight`/`BlockLightWorkspace` components on the message's
//! `chunk` entity also emit a warning and skip — defensive against any
//! lifecycle-ordering hazard.

use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Added, Commands, Entity, ParamSet, Query, Res, With};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{
    Column, ColumnIndex, Heightmaps, InColumn, ColumnChunks, ChunkLookup,
};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;

use crate::bfs::{
    normal_of, pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_RECHECK_LEVEL, FLAG_WRITE_LEVEL,
};
use crate::geom::face_cell_to_chunk_xyz;
use crate::heightmap::topmost_surface_world_y;
use crate::components::{
    BlockIncoming, BlockLight, BlockLightWorkspace, BlockPendingEgress, ChunkNeedsInitialLight,
    LightDirty, NeedsFullReseed, SkyIncoming, SkyLight, SkyLightSeededAsTopmost, SkyLightWorkspace,
    SkyPendingEgress, Wavefront,
};
use crate::nibble::NibbleArray;
use crate::storage::LightStorage;
use crate::table::{flag_bits, BlockLightTable};

pub fn enqueue_block_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut chunks: Query<(&mut BlockLight, &mut BlockLightWorkspace)>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((mut light, mut workspace)) = chunks.get_mut(placed.chunk) else {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "BlockPlaced.chunk missing BlockLight/BlockLightWorkspace; lifecycle ordering hazard"
            );
            continue;
        };

        let old_emission = table.emission_for(placed.old_state);
        let new_emission = table.emission_for(placed.new_state);
        let old_dampening = table.dampening_for(placed.old_state);
        let new_dampening = table.dampening_for(placed.new_state);

        if old_emission == new_emission && old_dampening != new_dampening {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "dampening-only change not yet handled; light will desync until cross-chunk distribute lands"
            );
            continue;
        }

        let x = placed.block_pos.x.rem_euclid(16) as u8;
        let y = placed.block_pos.y.rem_euclid(16) as u8;
        let z = placed.block_pos.z.rem_euclid(16) as u8;

        let mut pushed = false;

        if old_emission > new_emission {
            // The decrease BFS only walks neighbours, so the seed cell itself
            // must be cleared up front; otherwise the source position keeps
            // its previous emitted level after the emitter is removed.
            light.0.set(x as usize, y as usize, z as usize, 0);

            workspace.decrease_queue.push(pack_bfs_entry(
                x,
                z,
                y,
                old_emission,
                ALL_DIRECTIONS_BITSET,
                0,
            ));
            pushed = true;
        }

        if new_emission > 0 {
            // `FLAG_WRITE_LEVEL` makes the BFS write the source cell to
            // `new_emission` before stepping outward, so the source position
            // is established before any neighbour is reached.
            workspace.increase_queue.push(pack_bfs_entry(
                x,
                z,
                y,
                new_emission,
                ALL_DIRECTIONS_BITSET,
                FLAG_WRITE_LEVEL,
            ));
            pushed = true;
        }

        if pushed {
            commands.entity(placed.chunk).insert(LightDirty);
        }
    }
}

/// Seeds the top face of every newly-attached topmost-of-column chunk
/// with 256 BFS entries at `(x, z, 15)` level `15` carrying `FLAG_WRITE_LEVEL`.
///
/// "Topmost of column" is decided against the column's `ColumnChunks`:
/// `chunk_pos.y == min_chunk_y + sections.len() - 1`. Non-topmost chunks
/// (lower in the column) seed nothing — sky light reaches them only via the
/// downward BFS step from the chunk above. Chunks in skyless dimensions
/// never receive a `SkyLight` component (the `SkyLightBundle` insertion gate
/// in `lifecycle::attach_lighting_state` keys on `HasSkyLight`), so the
/// `Added<SkyLight>` filter self-gates this system.
///
/// Skips when `SkyLight` storage is already non-`Null`: the heightmap
/// fast-path in `seed_initial_light` initialises the topmost chunk to
/// `LightStorage::Uniform(15)` directly when the column's surface lies
/// fully below the chunk. Pushing 256 BFS seeds in that case would re-
/// arm the column-walker fast path in `propagate_increase_sky_system` and
/// emit 1280 cross-chunk wavefronts to every neighbour, restarting the
/// multi-chunk cascade the heightmap fast-path was designed to
/// eliminate. The `.after(seed_initial_light)` ordering on the system
/// registration in `plugin.rs` guarantees this gate sees the fast-path's
/// storage state.
pub fn enqueue_sky_light_initial(
    mut newly_added: Query<
        (Entity, &ChunkPos, &InColumn, &mut SkyLightWorkspace, &SkyLight),
        Added<SkyLight>,
    >,
    columns: Query<&ColumnChunks>,
    mut commands: Commands,
) {
    for (chunk_entity, chunk_pos, in_column, mut workspace, sky_light) in newly_added.iter_mut() {
        if !matches!(sky_light.0, LightStorage::Null) {
            continue;
        }
        let Ok(chunk_index) = columns.get(in_column.0) else {
            continue;
        };
        let top_chunk_y =
            chunk_index.min_section_y + chunk_index.sections.len() as i32 - 1;
        if chunk_pos.y != top_chunk_y {
            continue;
        }

        workspace.increase_queue.reserve(256);
        for z in 0..16u8 {
            for x in 0..16u8 {
                workspace.increase_queue.push(pack_bfs_entry(
                    x,
                    z,
                    15,
                    15,
                    ALL_DIRECTIONS_BITSET,
                    FLAG_WRITE_LEVEL,
                ));
            }
        }
        commands.entity(chunk_entity).insert(LightDirty);
    }
}

/// Reacts to `BlockPlaced` by enqueuing sky-light decrease and increase seeds
/// whenever the placed block changes either its dampening or its
/// `PROPAGATES_SKYLIGHT_DOWN` flag.
///
/// Missing `SkyLight`/`SkyLightWorkspace` components on the target chunk
/// emit a `tracing::warn!` and skip without panic; this defends against
/// `BlockPlaced` reaching a skyless-dim chunk (where the bundle is never
/// attached) or arriving before the lighting bundle insertion has flushed.
pub fn enqueue_sky_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut chunks: Query<(
        &mut SkyLight,
        &mut SkyLightWorkspace,
        &ChunkPos,
        &InColumn,
    )>,
    columns: Query<&ColumnChunks>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((mut light, mut workspace, chunk_pos, in_column)) =
            chunks.get_mut(placed.chunk)
        else {
            tracing::warn!(
                chunk = ?placed.chunk,
                block_pos = ?placed.block_pos,
                "BlockPlaced.chunk missing SkyLight/SkyLightWorkspace; skipping sky enqueue"
            );
            continue;
        };

        let old_dampening = table.dampening_for(placed.old_state);
        let new_dampening = table.dampening_for(placed.new_state);
        let old_flags = table.flags_for(placed.old_state);
        let new_flags = table.flags_for(placed.new_state);

        let occlusion_changed = !std::ptr::eq(
            table.occlusion_for(placed.old_state) as *const _,
            table.occlusion_for(placed.new_state) as *const _,
        );

        let sky_changed = old_dampening != new_dampening
            || (old_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN)
                != (new_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN)
            || occlusion_changed;
        if !sky_changed {
            continue;
        }

        let is_topmost = match columns.get(in_column.0) {
            Ok(chunk_index) => {
                let top_chunk_y =
                    chunk_index.min_section_y + chunk_index.sections.len() as i32 - 1;
                chunk_pos.y == top_chunk_y
            }
            Err(_) => false,
        };

        let x = placed.block_pos.x.rem_euclid(16) as u8;
        let y = placed.block_pos.y.rem_euclid(16) as u8;
        let z = placed.block_pos.z.rem_euclid(16) as u8;

        let stored = light.0.get(x as usize, y as usize, z as usize);
        let opacity_rose = new_dampening > old_dampening
            || ((old_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN) != 0
                && (new_flags & flag_bits::PROPAGATES_SKYLIGHT_DOWN) == 0);
        // The decrease BFS only walks neighbours, so the seed cell must be
        // cleared up front whenever the post-change opacity can only fall
        // below `stored`; otherwise the source position keeps its previous
        // sky-light level even though the new block opaquifies the cell.
        if opacity_rose && stored > 0 {
            light.0.set(x as usize, y as usize, z as usize, 0);
        }
        workspace.decrease_queue.push(pack_bfs_entry(
            x,
            z,
            y,
            stored,
            ALL_DIRECTIONS_BITSET,
            0,
        ));

        if y == 15 && is_topmost {
            workspace.increase_queue.push(pack_bfs_entry(
                x,
                z,
                15,
                15,
                ALL_DIRECTIONS_BITSET,
                FLAG_WRITE_LEVEL,
            ));
        } else {
            for d in [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ] {
                let (dx, dy, dz) = normal_of(d);
                let nx = x as i8 + dx;
                let ny = y as i8 + dy;
                let nz = z as i8 + dz;
                if !(0..16).contains(&nx)
                    || !(0..16).contains(&ny)
                    || !(0..16).contains(&nz)
                {
                    continue;
                }
                let neighbour_level =
                    light.0.get(nx as usize, ny as usize, nz as usize);
                workspace.increase_queue.push(pack_bfs_entry(
                    nx as u8,
                    nz as u8,
                    ny as u8,
                    neighbour_level,
                    ALL_DIRECTIONS_BITSET,
                    FLAG_RECHECK_LEVEL,
                ));
            }
        }

        commands.entity(placed.chunk).insert(LightDirty);
    }
}

const CARDINAL_DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Resolves a neighbour chunk by walking through `ColumnChunks` (for Y-axis
/// neighbours) or `ColumnIndex` + `ColumnChunks` (for X/Z-axis neighbours).
/// Returns `Some(entity)` only for `ChunkLookup::Loaded(entity)`; padding and
/// out-of-range and unloaded neighbours all return `None`.
fn resolve_loaded_neighbor(
    face: Direction,
    chunk_pos: ChunkPos,
    in_col: Entity,
    in_dim: Entity,
    column_indexes: &Query<&ColumnIndex>,
    chunk_indexes: &Query<&ColumnChunks>,
) -> Option<Entity> {
    match face {
        Direction::Up | Direction::Down => {
            let chunk_index = chunk_indexes.get(in_col).ok()?;
            let dy = if face == Direction::Up { 1 } else { -1 };
            match chunk_index.lookup(chunk_pos.y + dy) {
                ChunkLookup::Loaded(e) => Some(e),
                _ => None,
            }
        }
        _ => {
            let column_index = column_indexes.get(in_dim).ok()?;
            let (nx, nz) = match face {
                Direction::North => (chunk_pos.x, chunk_pos.z - 1),
                Direction::South => (chunk_pos.x, chunk_pos.z + 1),
                Direction::West => (chunk_pos.x - 1, chunk_pos.z),
                Direction::East => (chunk_pos.x + 1, chunk_pos.z),
                _ => unreachable!(),
            };
            let neighbour_col_pos =
                mcrs_engine::world::column::ColumnPos::new(nx, nz);
            let slot = column_index.0.get(&neighbour_col_pos)?;
            let neighbour_chunk_index = chunk_indexes.get(slot.entity).ok()?;
            match neighbour_chunk_index.lookup(chunk_pos.y) {
                ChunkLookup::Loaded(e) => Some(e),
                _ => None,
            }
        }
    }
}

/// Consumes `ChunkNeedsInitialLight` per chunk: scans the palette for
/// block-light emitters and seeds `BlockLightWorkspace::increase_queue`; on a
/// sky-having dimension's topmost chunk, additionally seeds 256
/// `SkyLightWorkspace::increase_queue` entries at y=15. On retopping, also
/// drives a decrease wave through the previously-topmost chunk.
///
/// The query is gated on `Option<Res<BlockLightTable>>` for consistency with
/// `prime_heightmaps_on_column_spawn` — early-returns if the resource has not
/// been built yet (registry freeze races early ticks).
pub fn seed_initial_light(
    table: Option<Res<BlockLightTable>>,
    sky_dims: Query<(), With<HasSkyLight>>,
    chunk_indexes: Query<&ColumnChunks>,
    heightmaps: Query<&Heightmaps>,
    mut chunks: ParamSet<(
        Query<
            (
                Entity,
                &BlockPalette,
                &InColumn,
                &InDimension,
                &ChunkPos,
                &mut BlockLightWorkspace,
                Option<&mut SkyLightWorkspace>,
                Option<&mut SkyLight>,
            ),
            With<ChunkNeedsInitialLight>,
        >,
        Query<
            (Entity, &ChunkPos, &InColumn, &mut SkyLightWorkspace, &SkyLight),
            With<SkyLightSeededAsTopmost>,
        >,
    )>,
    mut commands: Commands,
) {
    let Some(table) = table else {
        return;
    };

    // First pass: collect what needs to happen, since we can't hold p0() and
    // p1() borrows simultaneously. For each chunk in p0(), determine block
    // emitters, sky seeding, and a "previously-topmost invalidate" target.
    struct Plan {
        column: Entity,
        seeded_topmost: bool,
        new_chunk_y: i32,
    }
    let mut plans: Vec<Plan> = Vec::new();

    {
        let mut p0 = chunks.p0();
        for (
            chunk_entity,
            palette,
            in_col,
            in_dim,
            chunk_pos,
            mut block_ws,
            mut sky_ws_opt,
            mut sky_light_opt,
        ) in p0.iter_mut()
        {
            // Block-light emitter scan. Always run the cell-by-cell scan: the
            // for_each_distinct_state check would only skip the 4096-cell loop
            // for chunks with zero emitters, which is the common case, but
            // BlockPalette doesn't expose a positions-of-state API so the
            // scan is the path of least new surface.
            let mut has_emitter = false;
            palette.for_each_distinct_state(|state| {
                if table.emission_for(state) > 0 {
                    has_emitter = true;
                }
            });
            if has_emitter {
                for y in 0..16i32 {
                    for z in 0..16i32 {
                        for x in 0..16i32 {
                            let state = palette.get(BlockPos::new(x, y, z));
                            let emission = table.emission_for(state);
                            if emission > 0 {
                                block_ws.increase_queue.push(pack_bfs_entry(
                                    x as u8,
                                    z as u8,
                                    y as u8,
                                    emission,
                                    ALL_DIRECTIONS_BITSET,
                                    FLAG_WRITE_LEVEL,
                                ));
                            }
                        }
                    }
                }
            }

            // Sky-light seeding: heightmap fast-path.
            //
            // For sky-having dimensions, classify this chunk against the
            // column's primed `Heightmaps` so all-air chunks above the
            // surface skip BFS entirely (Case A: Uniform(15)), straddling
            // chunks fill above-surface cells directly and seed BFS only
            // at the surface line (Case B), and all-below chunks stay at
            // 0 (Case C). Replaces the prior unconditional 256-seed push at
            // y=15 on the topmost chunk, which forced an N-chunk cascade
            // through every air chunk above the surface during initial
            // load. Vanilla Starlight uses the same strategy via
            // `tryPropagateSkylight`.
            let dim_has_sky = sky_dims.get(in_dim.0).is_ok();
            let is_topmost = chunk_indexes
                .get(in_col.0)
                .ok()
                .map(|si| chunk_pos.y == si.min_section_y + si.sections.len() as i32 - 1)
                .unwrap_or(false);

            let mut sky_seeded = false;
            let mut seeded_topmost = false;

            if dim_has_sky {
                if let Some(sky_ws) = sky_ws_opt.as_deref_mut() {
                    let chunk_base_y = chunk_pos.y * 16;
                    let chunk_top_y = chunk_base_y + 15;

                    match heightmaps.get(in_col.0) {
                        Ok(hm) => {
                            let mut all_above = true;
                            let mut all_below = true;
                            for z in 0..16usize {
                                for x in 0..16usize {
                                    match topmost_surface_world_y(hm, x, z) {
                                        None => {
                                            all_below = false;
                                        }
                                        Some(s) => {
                                            if s > chunk_base_y {
                                                all_above = false;
                                            }
                                            if s <= chunk_top_y {
                                                all_below = false;
                                            }
                                        }
                                    }
                                }
                            }

                            if all_above {
                                // Case A: every column's first-air-above-surface
                                // is at or below this chunk's base. All 4096
                                // cells are air at level 15. Store the compressed
                                // Uniform(15) form and skip LightDirty — there is
                                // no work to converge.
                                if chunk_base_y <= 0 {
                                    // Cave-or-deeper chunks must never reach
                                    // Case A on a real overworld. If they do,
                                    // the column's heightmap is at sentinel
                                    // when seed_initial_light fired — capture
                                    // the offending column for diagnosis.
                                    let min_y = hm.min_y();
                                    let mut sample = [0i32; 9];
                                    let pts = [
                                        (0usize, 0usize), (15, 0), (0, 15), (15, 15),
                                        (7, 7), (0, 7), (15, 7), (7, 0), (7, 15),
                                    ];
                                    let mut sentinel_count = 0u16;
                                    let mut min_read = i32::MAX;
                                    let mut max_read = i32::MIN;
                                    for z in 0..16usize {
                                        for x in 0..16usize {
                                            let s = hm.surface_get(x, z);
                                            min_read = min_read.min(s);
                                            max_read = max_read.max(s);
                                            if s == min_y { sentinel_count += 1; }
                                        }
                                    }
                                    for (i, (x, z)) in pts.iter().enumerate() {
                                        sample[i] = hm.surface_get(*x, *z);
                                    }
                                    tracing::warn!(
                                        target: "mcrs_lighting::case_a_cave",
                                        chunk_x = chunk_pos.x,
                                        chunk_y = chunk_pos.y,
                                        chunk_z = chunk_pos.z,
                                        chunk_base_y,
                                        chunk_top_y,
                                        column = ?in_col.0,
                                        chunk = ?chunk_entity,
                                        heightmap_min_y = min_y,
                                        heightmap_min_read = min_read,
                                        heightmap_max_read = max_read,
                                        heightmap_sentinel_count = sentinel_count,
                                        heightmap_sample = ?sample,
                                        "Case A fired for cave chunk: chunk will be Uniform(15) but is below y=0 — heightmap likely unprimed"
                                    );
                                }
                                if let Some(sky_light) = sky_light_opt.as_deref_mut() {
                                    sky_light.0 = LightStorage::Uniform(15);
                                }
                            } else if all_below {
                                // Case C: every column's surface is at or above
                                // this chunk's top. No sky light reaches here;
                                // storage stays Null (=0).
                            } else {
                                // Case B: straddling. Start from a uniform-15
                                // nibble array (single 2 KiB memset) and zero
                                // out only the below-surface cells per (x, z)
                                // column. For a typical surface chunk the
                                // dark region is at the bottom of a small
                                // subset of columns, so this is far cheaper
                                // than per-cell sets of the lit region.
                                // Seed BFS at every lit y-level per column
                                // so the wavefront reaches dark cells at any
                                // height in adjacent columns (overhangs,
                                // multi-level cave pockets). Seeding only
                                // the surface-transition cell leaves the
                                // already-lit cells above it without BFS
                                // entries; the BFS cheap-out skips them
                                // before emitting horizontal wavefronts.
                                // Flags=0: storage is already at 15 for lit
                                // cells, no write needed.
                                let mut arr = NibbleArray::filled(15);
                                for z in 0..16usize {
                                    for x in 0..16usize {
                                        let s_opt = topmost_surface_world_y(hm, x, z);
                                        let max_dark_local_y = match s_opt {
                                            Some(s) if s > chunk_base_y => {
                                                (s - chunk_base_y).min(16) as usize
                                            }
                                            _ => 0,
                                        };
                                        for y_local in 0..max_dark_local_y {
                                            arr.set(x, y_local, z, 0);
                                        }
                                        // Only seed columns with lit cells in
                                        // this chunk. Fully-dark columns
                                        // (s > chunk_top_y) have storage=0 for
                                        // every cell; seeding them at level 15
                                        // would produce false level-15 seeds that
                                        // propagate outward at the wrong attenuation.
                                        // Those columns receive light from adjacent
                                        // lit columns via the BFS or cross-chunk pull.
                                        // Unsurfaced columns are entirely lit up to
                                        // chunk_top_y; the None arm matches the
                                        // original behaviour where the sentinel
                                        // (s == min_y) compared as s <= chunk_top_y.
                                        let lit_in_chunk =
                                            s_opt.map_or(true, |s| s <= chunk_top_y);
                                        if lit_in_chunk {
                                            let first_seed_y: u8 = match s_opt {
                                                Some(s) if s >= chunk_base_y => {
                                                    (s - chunk_base_y) as u8
                                                }
                                                _ => 0,
                                            };
                                            for y_seed_local in first_seed_y..=15u8 {
                                                sky_ws.increase_queue.push(pack_bfs_entry(
                                                    x as u8,
                                                    z as u8,
                                                    y_seed_local,
                                                    15,
                                                    ALL_DIRECTIONS_BITSET,
                                                    0,
                                                ));
                                            }
                                            sky_seeded = true;
                                        }
                                    }
                                }
                                if let Some(sky_light) = sky_light_opt.as_deref_mut() {
                                    sky_light.0 = LightStorage::Mixed(Box::new(arr));
                                }
                            }

                            if is_topmost {
                                commands
                                    .entity(chunk_entity)
                                    .insert(SkyLightSeededAsTopmost);
                                seeded_topmost = true;
                            }
                        }
                        Err(_) => {
                            // Defensive fallback when `Heightmaps` is missing.
                            // Reproduces the pre-fast-path behaviour: only the
                            // topmost chunk gets 256 seeds at y=15.
                            if is_topmost {
                                sky_ws.increase_queue.reserve(256);
                                for z in 0..16u8 {
                                    for x in 0..16u8 {
                                        sky_ws.increase_queue.push(pack_bfs_entry(
                                            x,
                                            z,
                                            15,
                                            15,
                                            ALL_DIRECTIONS_BITSET,
                                            FLAG_WRITE_LEVEL,
                                        ));
                                    }
                                }
                                commands
                                    .entity(chunk_entity)
                                    .insert(SkyLightSeededAsTopmost);
                                sky_seeded = true;
                                seeded_topmost = true;
                            }
                        }
                    }
                }
            }

            // Only chunks that actually seeded work (block emitters or a
            // straddling-surface sky seed) need `LightDirty`. Case A writes
            // Uniform(15) directly and Case C leaves storage at 0 — neither
            // requires the converge loop, so neither marks `LightDirty`.
            // `distribute_*` will re-mark any of these as soon as an actual
            // wavefront reaches them.
            if has_emitter || sky_seeded {
                commands.entity(chunk_entity).insert(LightDirty);
            }
            commands
                .entity(chunk_entity)
                .remove::<ChunkNeedsInitialLight>();

            plans.push(Plan {
                column: in_col.0,
                seeded_topmost,
                new_chunk_y: chunk_pos.y,
            });
        }
    }

    // Second pass: for each plan that seeded a new topmost, find any previously-
    // topmost chunk in the SAME column with a lower chunk_pos.y and walk a
    // decrease wave through its top face using the stored sky levels. The pass
    // owns the &mut SkyLightWorkspace on the previous-topmost entity here
    // exclusively, since the first pass already released its borrow.
    if plans.iter().any(|p| p.seeded_topmost) {
        let mut p1 = chunks.p1();
        for plan in &plans {
            if !plan.seeded_topmost {
                continue;
            }
            for (prev_entity, prev_chunk_pos, prev_in_col, mut prev_sky_ws, prev_sky_light) in
                p1.iter_mut()
            {
                if prev_in_col.0 != plan.column {
                    continue;
                }
                if prev_chunk_pos.y >= plan.new_chunk_y {
                    continue;
                }
                for z in 0..16u8 {
                    for x in 0..16u8 {
                        let stored = prev_sky_light.0.get(x as usize, 15, z as usize);
                        prev_sky_ws.decrease_queue.push(pack_bfs_entry(
                            x,
                            z,
                            15,
                            stored,
                            ALL_DIRECTIONS_BITSET,
                            0,
                        ));
                    }
                }
                commands
                    .entity(prev_entity)
                    .remove::<SkyLightSeededAsTopmost>();
                commands.entity(prev_entity).insert(LightDirty);
            }
        }
    }
}

/// Consumes `Added<ChunkLoaded>` per chunk: reads each loaded cardinal
/// neighbour's face cells into the new chunk's `*Incoming`, then drains any
/// `*PendingEgress` entries that the neighbour buffered while we were
/// unloaded. Marks the new chunk and every touched loaded neighbour
/// `LightDirty`.
pub fn pull_neighbor_edge_levels(
    table: Option<Res<BlockLightTable>>,
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension, &InColumn), Added<ChunkLoaded>>,
    column_indexes: Query<&ColumnIndex>,
    chunk_indexes: Query<&ColumnChunks>,
    block_light_read: Query<&BlockLight>,
    sky_light_read: Query<&SkyLight>,
    mut block_pending: Query<&mut BlockPendingEgress>,
    mut sky_pending: Query<&mut SkyPendingEgress>,
    mut block_incoming: Query<&mut BlockIncoming>,
    mut sky_incoming: Query<&mut SkyIncoming>,
    mut commands: Commands,
) {
    if table.is_none() {
        return;
    }

    // All newly-loaded chunk entities this tick. Pull only makes sense
    // for bootstrapping a new chunk against an *already-settled*
    // neighbour. When a neighbour is also brand-new, it was just processed
    // by `seed_initial_light` (heightmap fast-path), and the natural
    // egress→distribute cascade in `LightConvergeSchedule` will route
    // wavefronts between them if needed. Pulling redundantly fires a 256-
    // entry incoming buffer + `LightDirty` marker for a chunk that
    // otherwise had no work — and on stone-capped boundary cells that
    // re-arms a 6-iteration converge cascade.
    let newly_loaded_set: rustc_hash::FxHashSet<Entity> =
        newly_loaded.iter().map(|(e, _, _, _)| e).collect();

    for (new_chunk, chunk_pos, in_dim, in_col) in newly_loaded.iter() {
        // Tracks whether anything was actually pushed into the new chunk's
        // `BlockIncoming` / `SkyIncoming`. Without a payload there is no
        // pending cascade work, so `LightDirty` would just force a no-op pass
        // through the par-iter scan in the convergence sub-schedule.
        let mut new_chunk_has_incoming = false;

        // Cell-level pull cannot beat a `Uniform(15)` destination, so a
        // pre-check on the new chunk's storage lets us skip the per-face
        // 256-cell read loop entirely when the heightmap fast-path has
        // already filled the chunk to max. Cached once per new_chunk.
        let new_sky_already_max = sky_light_read
            .get(new_chunk)
            .ok()
            .map(|sl| matches!(sl.0, LightStorage::Uniform(15)))
            .unwrap_or(false);
        let new_block_already_max = block_light_read
            .get(new_chunk)
            .ok()
            .map(|bl| matches!(bl.0, LightStorage::Uniform(15)))
            .unwrap_or(false);

        for face in CARDINAL_DIRECTIONS {
            let Some(neighbour_entity) = resolve_loaded_neighbor(
                face,
                *chunk_pos,
                in_col.0,
                in_dim.0,
                &column_indexes,
                &chunk_indexes,
            ) else {
                continue;
            };

            // Skip neighbours that were also Added<ChunkLoaded> this tick,
            // UNLESS the neighbour already has settled Uniform(15) sky light.
            // A Uniform(15) chunk (Case A: all columns fully above this
            // chunk's top) is written directly by seed_initial_light with no
            // BFS work remaining — its storage is final and must be pulled now
            // so the dark (Case B/C) new chunk receives the correct initial
            // wavefront. For all other newly-loaded neighbours, the
            // egress→distribute cascade handles any actual flow between fresh
            // chunks during convergence.
            if newly_loaded_set.contains(&neighbour_entity) {
                let neighbour_sky_is_uniform_15 = sky_light_read
                    .get(neighbour_entity)
                    .map(|sl| matches!(sl.0, LightStorage::Uniform(15)))
                    .unwrap_or(false);
                if !neighbour_sky_is_uniform_15 {
                    continue;
                }
            }

            // `face` is the direction from us (new chunk) to the neighbour
            // in OUR (destination) frame, so it doubles as the incoming face
            // index. `from_face` is the neighbour's frame face pointing back
            // at us; we use it both to compute the neighbour's face-cell
            // coordinates and to filter the neighbour's pending-egress entries
            // (which are tagged in the neighbour's frame).
            let from_face = face.opposite();
            let dest_face = face.index() as u8;
            let neighbour_expected_face = from_face.index() as u8;

            // Per-face tracker for the optional neighbour `LightDirty` insert.
            // Marking a quiescent neighbour dirty without having drained
            // any wavefronts from it just forces a no-op converge pass
            // through it; the natural egress→distribute path re-dirties
            // the neighbour later if our new chunk actually emits
            // anything.
            let mut drained_pending_from_neighbour = false;

            // Read neighbour's face cells into incoming with Manhattan-1
            // pre-attenuation. Skipped per light-channel when the new
            // chunk's storage is already `Uniform(15)`, since any pulled
            // level ≤ 14 cannot improve on a max-stored cell.
            if !new_sky_already_max || !new_block_already_max {
                for cell_a in 0..16u8 {
                    for cell_b in 0..16u8 {
                        let (nx, ny, nz) =
                            face_cell_to_chunk_xyz(from_face, cell_a, cell_b);

                        if !new_block_already_max {
                            if let Ok(bl) = block_light_read.get(neighbour_entity) {
                                let level = bl.0.get(nx as usize, ny as usize, nz as usize);
                                if level > 0 {
                                    let attenuated = level.saturating_sub(1);
                                    if let Ok(mut inc) = block_incoming.get_mut(new_chunk) {
                                        inc.0.push(Wavefront::new(
                                            dest_face, cell_a, cell_b, attenuated,
                                        ));
                                        new_chunk_has_incoming = true;
                                    }
                                }
                            }
                        }

                        if !new_sky_already_max {
                            if let Ok(sl) = sky_light_read.get(neighbour_entity) {
                                let level = sl.0.get(nx as usize, ny as usize, nz as usize);
                                if level > 0 {
                                    let attenuated = level.saturating_sub(1);
                                    if let Ok(mut inc) = sky_incoming.get_mut(new_chunk) {
                                        inc.0.push(Wavefront::new(
                                            dest_face, cell_a, cell_b, attenuated,
                                        ));
                                        new_chunk_has_incoming = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Drain neighbour's *PendingEgress entries addressed back at us.
            // The neighbour buffered wavefronts with `face` in the neighbour's
            // frame; an entry targets us iff its face equals
            // neighbour_expected_face (the neighbour's face pointing at us).

            if let Ok(mut pending) = block_pending.get_mut(neighbour_entity) {
                if !pending.0.is_empty() {
                    pending.0.retain(|w| {
                        if w.face() == neighbour_expected_face {
                            if let Ok(mut inc) = block_incoming.get_mut(new_chunk) {
                                inc.0.push(Wavefront::new(
                                    dest_face,
                                    w.cell_x(),
                                    w.cell_z(),
                                    w.level(),
                                ));
                                new_chunk_has_incoming = true;
                                drained_pending_from_neighbour = true;
                            }
                            false
                        } else {
                            true
                        }
                    });
                }
            }

            if let Ok(mut pending) = sky_pending.get_mut(neighbour_entity) {
                if !pending.0.is_empty() {
                    pending.0.retain(|w| {
                        if w.face() == neighbour_expected_face {
                            if let Ok(mut inc) = sky_incoming.get_mut(new_chunk) {
                                inc.0.push(Wavefront::new(
                                    dest_face,
                                    w.cell_x(),
                                    w.cell_z(),
                                    w.level(),
                                ));
                                new_chunk_has_incoming = true;
                                drained_pending_from_neighbour = true;
                            }
                            false
                        } else {
                            true
                        }
                    });
                }
            }

            // Mark the neighbour dirty only when the pull actually changed
            // its state (drained an entry from its pending egress). A pure
            // read of the neighbour's face cells is non-mutating, so the
            // neighbour does not need to converge on our behalf; if our new
            // chunk emits wavefronts outward later, `distribute_*` will
            // re-dirty the neighbour at that point.
            if drained_pending_from_neighbour {
                commands.entity(neighbour_entity).insert(LightDirty);
            }
        }

        // Only mark the new chunk dirty if a neighbour actually pushed
        // something into its incoming buffer. An isolated load with no loaded
        // neighbours has no pending cascade work, and `distribute_*` will
        // re-mark the chunk as soon as wavefronts arrive later.
        if new_chunk_has_incoming {
            commands.entity(new_chunk).insert(LightDirty);
        }
    }
}

/// Consumes `Added<NeedsFullReseed>` on `Column` entities: iterates the
/// column's `ColumnChunks.sections` slots and re-inserts
/// `ChunkNeedsInitialLight` on every loaded chunk in the column. Removes
/// `NeedsFullReseed` from the column entity.
///
/// Gated on `ColumnHeightmapScan::is_finalized()`. When the scan is not yet
/// finalized, `Heightmaps::surface_get` returns the sentinel `min_y` for
/// every unclosed XZ column. Re-marking chunks with `ChunkNeedsInitialLight`
/// in that state causes `seed_initial_light` to misclassify cave chunks as
/// Case A (Uniform(15)). The natural lifecycle in
/// `prime_heightmaps_on_column_spawn` inserts the marker once the scan
/// finalizes with a correctly primed heightmap, so deferring is safe: the
/// in-flight wavefronts that triggered the overflow were entering a column
/// whose initial seed has not yet been computed, and that initial seed will
/// produce the correct lighting state from scratch once the heightmap is
/// closed.
pub fn consume_needs_full_reseed(
    newly_marked: Query<
        (Entity, &ColumnChunks, Option<&crate::lifecycle::ColumnHeightmapScan>),
        (With<Column>, Added<NeedsFullReseed>),
    >,
    mut commands: Commands,
) {
    for (column_entity, chunk_index, scan_opt) in newly_marked.iter() {
        let loaded = chunk_index.sections.iter().filter(|s| s.is_some()).count();
        let total = chunk_index.sections.len();
        let scan_finalized = scan_opt.map_or(false, |s| s.is_finalized());

        if !scan_finalized {
            tracing::warn!(
                target: "mcrs_lighting::consume_reseed",
                column = ?column_entity,
                chunks_loaded = loaded,
                chunks_total = total,
                scan_present = scan_opt.is_some(),
                "Dropping NeedsFullReseed: heightmap scan not finalized. The natural lifecycle in \
                 prime_heightmaps_on_column_spawn will insert ChunkNeedsInitialLight when the scan \
                 closes; reseeding now would read sentinel min_y and mis-Uniform(15) cave chunks."
            );
            commands.entity(column_entity).remove::<NeedsFullReseed>();
            continue;
        }

        for slot in chunk_index.sections.iter() {
            if let Some(chunk_entity) = slot {
                commands
                    .entity(*chunk_entity)
                    .insert(ChunkNeedsInitialLight);
            }
        }
        commands.entity(column_entity).remove::<NeedsFullReseed>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs::{
        unpack_bfs_entry_flags, unpack_bfs_entry_level, unpack_bfs_entry_x, unpack_bfs_entry_y,
        unpack_bfs_entry_z,
    };
    use bevy_app::{App, Update};
    use bevy_ecs::message::Messages;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_engine::world::column::{Column, ColumnPos, ColumnIndex, ColumnSlot, InColumn, ColumnChunks};
    use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
    use mcrs_lighting_table_helpers::*;
    use mcrs_minecraft_block::block::BlockUpdateFlags;
    use mcrs_protocol::BlockStateId;

    mod mcrs_lighting_table_helpers {
        use super::*;
        use crate::table::{flag_bits, BlockLightTable};

        pub const AIR: BlockStateId = BlockStateId(0);
        pub const STONE: BlockStateId = BlockStateId(1);
        pub const TORCH_HI: BlockStateId = BlockStateId(2);
        pub const TORCH_LO: BlockStateId = BlockStateId(3);
        pub const LEAVES: BlockStateId = BlockStateId(4);

        pub fn make_test_table() -> BlockLightTable {
            let state_count = 5usize;
            let mut emission = vec![0u8; state_count].into_boxed_slice();
            let mut dampening = vec![0u8; state_count].into_boxed_slice();
            let occlusion: Box<[&'static VoxelShape]> =
                vec![VoxelShape::empty(); state_count].into_boxed_slice();
            let mut flags = vec![0u8; state_count].into_boxed_slice();

            emission[AIR.0 as usize] = 0;
            dampening[AIR.0 as usize] = 0;
            flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[STONE.0 as usize] = 0;
            dampening[STONE.0 as usize] = 0;
            flags[STONE.0 as usize] = 0;

            emission[TORCH_HI.0 as usize] = 14;
            dampening[TORCH_HI.0 as usize] = 0;
            flags[TORCH_HI.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[TORCH_LO.0 as usize] = 7;
            dampening[TORCH_LO.0 as usize] = 0;
            flags[TORCH_LO.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

            emission[LEAVES.0 as usize] = 0;
            dampening[LEAVES.0 as usize] = 1;
            flags[LEAVES.0 as usize] = flag_bits::IS_NOT_AIR;

            BlockLightTable {
                emission,
                dampening,
                occlusion,
                flags,
            }
        }
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_block_light_on_block_placed);
        app
    }

    fn spawn_chunk(app: &mut App) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn((BlockLight::default(), BlockLightWorkspace::default()))
            .id()
    }

    fn write_placed(app: &mut App, placed: BlockPlaced) {
        app.world_mut()
            .resource_mut::<Messages<BlockPlaced>>()
            .write(placed);
    }

    fn block_placed(
        chunk: bevy_ecs::entity::Entity,
        block_pos: BlockPos,
        old_state: BlockStateId,
        new_state: BlockStateId,
    ) -> BlockPlaced {
        BlockPlaced {
            chunk,
            chunk_pos: ChunkPos::new(0, 0, 0),
            block_pos,
            old_state,
            new_state,
            flags: BlockUpdateFlags::empty(),
        }
    }

    #[test]
    fn enqueue_increase_on_emitter_placed() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 5, 9), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.increase_queue.len(), 1, "one increase seed");
        assert!(
            workspace.decrease_queue.is_empty(),
            "no decrease seed for 0 → 14"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty inserted"
        );
    }

    #[test]
    fn enqueue_decrease_on_emitter_removed() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 8, 8), TORCH_HI, AIR),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.decrease_queue.len(), 1, "one decrease seed");
        assert!(
            workspace.increase_queue.is_empty(),
            "no increase seed for 14 → 0"
        );
        let entry = workspace.decrease_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 8);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 8);
        assert_eq!(unpack_bfs_entry_z(entry), 8);
        assert_eq!(unpack_bfs_entry_level(entry), 14);
        assert!(app.world().get::<LightDirty>(entity).is_some());
    }

    #[test]
    fn enqueue_both_on_swap() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(1, 2, 3), TORCH_HI, TORCH_LO),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.decrease_queue.len(), 1);
        assert_eq!(workspace.increase_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(workspace.decrease_queue[0]),
            14,
            "decrease at old emission"
        );
        assert_eq!(
            unpack_bfs_entry_level(workspace.increase_queue[0]),
            7,
            "increase at new emission"
        );
        assert!(app.world().get::<LightDirty>(entity).is_some());
    }

    #[test]
    fn enqueue_no_op_on_zero_zero() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // AIR → STONE: both emission=0, both dampening=0 in the test table, so
        // the dampening-only-change branch does NOT trigger and the system
        // simply records no work.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, STONE),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(workspace.increase_queue.is_empty());
        assert!(workspace.decrease_queue.is_empty());
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on no-op"
        );
    }

    #[test]
    fn enqueue_dampening_only_change_warns() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // AIR (emission=0, dampening=0) → LEAVES (emission=0, dampening=1).
        // Pure dampening change; the system warns and skips.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert!(
            workspace.increase_queue.is_empty(),
            "dampening-only skips increase"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "dampening-only skips decrease"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on dampening-only change"
        );
    }

    #[test]
    fn enqueue_missing_components_warns() {
        let mut app = build_app();
        // Spawn an entity WITHOUT BlockLight/BlockLightWorkspace — emulates
        // a chunk the lighting lifecycle has not yet attached state to.
        let entity = app.world_mut().spawn(()).id();
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, TORCH_HI),
        );

        app.update();

        assert!(
            app.world().get::<BlockLightWorkspace>(entity).is_none(),
            "entity still has no workspace"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty must NOT be inserted on missing components"
        );
    }

    #[test]
    fn enqueue_uses_rem_euclid_for_negative_coords() {
        let mut app = build_app();
        let entity = spawn_chunk(&mut app);
        // BlockPos::new(-3, 5, -19) — rem_euclid(16) yields (13, 5, 13).
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(-3, 5, -19), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<BlockLightWorkspace>(entity)
            .expect("workspace");
        assert_eq!(workspace.increase_queue.len(), 1);
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 13, "x = -3 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 5);
        assert_eq!(unpack_bfs_entry_z(entry), 13, "z = -19 rem_euclid 16 = 13");
        assert_eq!(unpack_bfs_entry_level(entry), 14);
    }

    fn build_sky_initial_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, enqueue_sky_light_initial);
        app
    }

    fn spawn_column_with_chunks(
        app: &mut App,
        min_chunk_y: i32,
        chunk_slots: Vec<Option<bevy_ecs::entity::Entity>>,
    ) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn(ColumnChunks {
                min_section_y: min_chunk_y,
                sections: chunk_slots.into_boxed_slice(),
            })
            .id()
    }

    #[test]
    fn enqueue_sky_initial_seeds_topmost_chunk_only() {
        let mut app = build_sky_initial_app();

        let chunk = app.world_mut().spawn_empty().id();
        let column = spawn_column_with_chunks(&mut app, 0, vec![Some(chunk)]);
        app.world_mut().entity_mut(chunk).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(chunk)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            256,
            "topmost-of-column chunk seeds 256 entries (16 x 16 at y=15)"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "initial seed does not push decrease entries"
        );
        for entry in &workspace.increase_queue {
            assert_eq!(unpack_bfs_entry_y(*entry) as u8, 15, "y == 15");
            assert_eq!(unpack_bfs_entry_level(*entry), 15, "level == 15");
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_WRITE_LEVEL,
                0,
                "FLAG_WRITE_LEVEL bit set on every seed"
            );
        }
        assert!(
            app.world().get::<LightDirty>(chunk).is_some(),
            "LightDirty inserted on topmost-of-column seed"
        );
    }

    #[test]
    fn enqueue_sky_initial_skips_non_topmost() {
        let mut app = build_sky_initial_app();

        let chunk_below = app.world_mut().spawn_empty().id();
        let chunk_topmost = app.world_mut().spawn_empty().id();
        // Two-chunk column: chunk-Y 0 (below) and chunk-Y 1 (topmost).
        let column = spawn_column_with_chunks(
            &mut app,
            0,
            vec![Some(chunk_below), Some(chunk_topmost)],
        );
        // Only the below chunk gets SkyLight added; topmost is left bare
        // so this single test does not also seed an unrelated chunk.
        app.world_mut().entity_mut(chunk_below).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(chunk_below)
            .expect("sky workspace");
        assert!(
            workspace.increase_queue.is_empty(),
            "non-topmost chunk seeds nothing"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "non-topmost chunk seeds no decrease"
        );
        assert!(
            app.world().get::<LightDirty>(chunk_below).is_none(),
            "LightDirty NOT inserted on non-topmost-of-column chunk"
        );
    }

    fn build_sky_on_placed_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        app
    }

    fn spawn_sky_chunk_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(ColumnChunks {
                min_section_y: 0,
                sections: vec![Some(chunk)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(chunk).insert((
            SkyLight::default(),
            SkyLightWorkspace::default(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
        ));
        chunk
    }

    fn spawn_sky_chunk_non_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let chunk = app.world_mut().spawn_empty().id();
        let dummy_topmost = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(ColumnChunks {
                min_section_y: 0,
                sections: vec![Some(chunk), Some(dummy_topmost)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(chunk).insert((
            SkyLight::default(),
            SkyLightWorkspace::default(),
            ChunkPos::new(0, 0, 0),
            InColumn(column),
        ));
        chunk
    }

    #[test]
    fn enqueue_sky_on_block_placed_pushes_decrease_and_neighbour_seeds() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        // AIR (damp=0, propagates) -> LEAVES (damp=1, no propagates flag);
        // sky_changed predicate trips on both dampening and flag delta.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 10, 8), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert!(
            !workspace.decrease_queue.is_empty(),
            "dampening change pushes a decrease seed"
        );
        assert!(
            !workspace.increase_queue.is_empty(),
            "y=10 (non-top) pushes neighbour-support increase seeds"
        );
        // y=10 (intra-chunk, not 15) -> six neighbour seeds.
        assert_eq!(
            workspace.increase_queue.len(),
            6,
            "y < 15 produces exactly six neighbour-support seeds"
        );
        for entry in &workspace.increase_queue {
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_RECHECK_LEVEL,
                0,
                "every neighbour seed carries FLAG_RECHECK_LEVEL"
            );
        }
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty inserted after dampening change"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_top_seeds_top_face() {
        // y == 15 path: a single top-face increase seed instead of six
        // neighbour seeds.
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            1,
            "y == 15 produces exactly one top-face seed"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 15);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 15);
        assert_ne!(
            unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "top-of-chunk seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_when_predicate_false() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        // AIR -> AIR: old_state == new_state, early-out before predicate.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(0, 0, 0), AIR, AIR),
        );
        // AIR -> TORCH_HI: both have dampening=0 AND PROPAGATES_SKYLIGHT_DOWN,
        // so sky_changed is false and the system continues without queueing.
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(1, 1, 1), AIR, TORCH_HI),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert!(workspace.increase_queue.is_empty());
        assert!(workspace.decrease_queue.is_empty());
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty NOT inserted on no-op sky enqueue"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_warns_missing_components() {
        use std::io;
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::fmt::MakeWriter;

        #[derive(Clone)]
        struct VecWriter(Arc<Mutex<Vec<u8>>>);

        impl io::Write for VecWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        impl<'a> MakeWriter<'a> for VecWriter {
            type Writer = VecWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let writer = VecWriter(Arc::clone(&captured));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let mut app = build_sky_on_placed_app();
            // Chunk without SkyLight/SkyLightWorkspace (skyless-dim shape).
            let entity = app
                .world_mut()
                .spawn((BlockLight::default(), BlockLightWorkspace::default()))
                .id();
            write_placed(
                &mut app,
                block_placed(entity, BlockPos::new(2, 3, 4), AIR, LEAVES),
            );

            app.update();

            assert!(
                app.world().get::<SkyLightWorkspace>(entity).is_none(),
                "entity still has no sky workspace"
            );
            assert!(
                app.world().get::<LightDirty>(entity).is_none(),
                "LightDirty must NOT be inserted when SkyLight is missing"
            );
        });

        let bytes = captured.lock().unwrap();
        let output = String::from_utf8_lossy(&bytes);
        assert!(
            output.contains("BlockPlaced.chunk missing SkyLight/SkyLightWorkspace"),
            "expected warn substring in captured tracing output, got: {output}"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_clears_seed_cell_on_opacity_rise() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        app.world_mut()
            .get_mut::<SkyLight>(entity)
            .expect("sky light")
            .0
            .set(8, 5, 8, 10);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), AIR, LEAVES),
        );

        app.update();

        let light = app.world().get::<SkyLight>(entity).expect("sky light");
        assert_eq!(
            light.0.get(8, 5, 8),
            0,
            "seed cell cleared because opacity rose"
        );
        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(workspace.decrease_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(workspace.decrease_queue[0]),
            10,
            "decrease seed carries pre-clear stored level"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_keeps_seed_cell_when_opacity_drops() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        app.world_mut()
            .get_mut::<SkyLight>(entity)
            .expect("sky light")
            .0
            .set(8, 5, 8, 3);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), LEAVES, AIR),
        );

        app.update();

        let light = app.world().get::<SkyLight>(entity).expect("sky light");
        assert_eq!(
            light.0.get(8, 5, 8),
            3,
            "seed cell unchanged because opacity did not rise"
        );
        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(workspace.decrease_queue.len(), 1);
        assert_eq!(
            unpack_bfs_entry_level(workspace.decrease_queue[0]),
            3,
            "decrease seed carries stored level"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_top_seed_when_not_topmost() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_non_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        // y=15 sits at the top of the chunk, so the Up neighbour at y=16
        // is outside the chunk and is skipped by the bounds guard. Five
        // neighbour-recheck seeds remain.
        assert_eq!(
            workspace.increase_queue.len(),
            5,
            "non-topmost chunk falls through to neighbour-recheck branch at y=15"
        );
        for entry in &workspace.increase_queue {
            assert_ne!(
                unpack_bfs_entry_flags(*entry) & FLAG_RECHECK_LEVEL,
                0,
                "every neighbour seed carries FLAG_RECHECK_LEVEL"
            );
            assert_eq!(
                unpack_bfs_entry_flags(*entry) & FLAG_WRITE_LEVEL,
                0,
                "no neighbour seed carries FLAG_WRITE_LEVEL"
            );
        }
    }

    #[test]
    fn enqueue_sky_on_block_placed_emits_top_seed_when_topmost() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            1,
            "topmost chunk emits a single top-face seed at y=15"
        );
        let entry = workspace.increase_queue[0];
        assert_eq!(unpack_bfs_entry_x(entry), 3);
        assert_eq!(unpack_bfs_entry_y(entry) as u8, 15);
        assert_eq!(unpack_bfs_entry_z(entry), 9);
        assert_eq!(unpack_bfs_entry_level(entry), 15);
        assert_ne!(
            unpack_bfs_entry_flags(entry) & FLAG_WRITE_LEVEL,
            0,
            "top-face seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_trips_on_occlusion_only_change() {
        const SHAPE_A: BlockStateId = BlockStateId(10);
        const SHAPE_B: BlockStateId = BlockStateId(11);

        let state_count = 12usize;
        let mut emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let mut occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();

        emission[AIR.0 as usize] = 0;
        dampening[AIR.0 as usize] = 0;
        flags[AIR.0 as usize] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

        // Two states share dampening and flag bits but project distinct
        // occlusion shapes. `dampening = 5` keeps `PROPAGATES_SKYLIGHT_DOWN`
        // cleared on both (matching the production `compute_flags` invariant)
        // so the dampening and flag arms of `sky_changed` stay silent and the
        // test exclusively exercises the occlusion-shape pointer comparison.
        dampening[SHAPE_A.0 as usize] = 5;
        dampening[SHAPE_B.0 as usize] = 5;
        flags[SHAPE_A.0 as usize] =
            flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::IS_NOT_AIR;
        flags[SHAPE_B.0 as usize] =
            flag_bits::IS_CONDITIONALLY_OPAQUE | flag_bits::IS_NOT_AIR;
        occlusion[SHAPE_A.0 as usize] = VoxelShape::empty();
        occlusion[SHAPE_B.0 as usize] = VoxelShape::block();

        let table = BlockLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        };

        assert!(
            !std::ptr::eq(
                table.occlusion_for(SHAPE_A) as *const _,
                table.occlusion_for(SHAPE_B) as *const _,
            ),
            "fixture must mint distinct occlusion shape pointers"
        );
        assert_eq!(table.dampening_for(SHAPE_A), table.dampening_for(SHAPE_B));
        assert_eq!(
            table.flags_for(SHAPE_A) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
            table.flags_for(SHAPE_B) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
        );

        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(table);
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        let entity = spawn_sky_chunk_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), SHAPE_A, SHAPE_B),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.decrease_queue.len(),
            1,
            "occlusion-only delta still pushes a decrease seed"
        );
        assert_eq!(
            workspace.increase_queue.len(),
            6,
            "y != 15 path enqueues six neighbour-recheck seeds"
        );
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "occlusion-only delta inserts LightDirty"
        );
    }

    fn build_seed_initial_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, seed_initial_light);
        app
    }

    fn spawn_palette_with_torches(positions: &[(i32, i32, i32)]) -> BlockPalette {
        let mut palette = BlockPalette::default();
        palette.fill(AIR);
        for &(x, y, z) in positions {
            palette.set(BlockPos::new(x, y, z), TORCH_HI);
        }
        palette
    }

    fn spawn_dimension(app: &mut App, with_sky: bool) -> bevy_ecs::entity::Entity {
        let mut e = app.world_mut().spawn(ColumnIndex::default());
        if with_sky {
            e.insert(HasSkyLight);
        }
        e.id()
    }

    fn spawn_topmost_chunk_for_seed(
        app: &mut App,
        dim: bevy_ecs::entity::Entity,
        palette: BlockPalette,
        sky: bool,
    ) -> (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
        let chunk = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let mut emut = app.world_mut().entity_mut(chunk);
        emut.insert((
            palette,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            ChunkNeedsInitialLight,
        ));
        if sky {
            emut.insert((SkyLight::default(), SkyLightWorkspace::default()));
        }
        (chunk, column)
    }

    #[test]
    fn seed_initial_light_emits_block_emitters_and_sky_source() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);
        let palette = spawn_palette_with_torches(&[
            (0, 0, 0),
            (5, 5, 5),
            (10, 1, 8),
            (3, 12, 7),
            (15, 15, 15),
        ]);
        let (chunk, _col) = spawn_topmost_chunk_for_seed(&mut app, dim, palette, true);

        app.update();

        let block_ws = app
            .world()
            .get::<BlockLightWorkspace>(chunk)
            .expect("block ws");
        assert_eq!(
            block_ws.increase_queue.len(),
            5,
            "five torches emit five increase seeds"
        );
        let sky_ws = app
            .world()
            .get::<SkyLightWorkspace>(chunk)
            .expect("sky ws");
        assert_eq!(
            sky_ws.increase_queue.len(),
            256,
            "topmost on sky-having dim seeds 256 sky entries"
        );
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk).is_some(),
            "SkyLightSeededAsTopmost inserted"
        );
        assert!(
            app.world().get::<LightDirty>(chunk).is_some(),
            "LightDirty inserted"
        );
        assert!(
            app.world().get::<ChunkNeedsInitialLight>(chunk).is_none(),
            "ChunkNeedsInitialLight removed"
        );
    }

    #[test]
    fn seed_initial_light_invalidates_previous_topmost_on_retopping() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);

        // Chunk A at chunk-Y 0 with SkyLightSeededAsTopmost already; stored
        // sky level 12 across the top face.
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut palette_a = BlockPalette::default();
        palette_a.fill(AIR);
        let mut a_sky_light = SkyLight::default();
        for z in 0..16usize {
            for x in 0..16usize {
                a_sky_light.0.set(x, 15, z, 12);
            }
        }
        app.world_mut().entity_mut(chunk_a).insert((
            palette_a,
            ChunkPos::new(0, 0, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            a_sky_light,
            SkyLightWorkspace::default(),
            SkyLightSeededAsTopmost,
        ));

        // Chunk B at chunk-Y 1 (the new topmost) needs initial light.
        let mut palette_b = BlockPalette::default();
        palette_b.fill(AIR);
        app.world_mut().entity_mut(chunk_b).insert((
            palette_b,
            ChunkPos::new(0, 1, 0),
            InColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            SkyLight::default(),
            SkyLightWorkspace::default(),
            ChunkNeedsInitialLight,
        ));

        app.update();

        // Previous topmost A: marker removed, LightDirty inserted, decrease
        // wave seeded with stored level 12.
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk_a).is_none(),
            "previous topmost's SkyLightSeededAsTopmost removed"
        );
        assert!(
            app.world().get::<LightDirty>(chunk_a).is_some(),
            "previous topmost marked LightDirty"
        );
        let a_ws = app
            .world()
            .get::<SkyLightWorkspace>(chunk_a)
            .expect("sky ws on A");
        assert_eq!(
            a_ws.decrease_queue.len(),
            256,
            "previous topmost gets 256 decrease seeds"
        );
        for entry in &a_ws.decrease_queue {
            assert_eq!(
                unpack_bfs_entry_level(*entry),
                12,
                "decrease seed carries stored level"
            );
            assert_eq!(unpack_bfs_entry_y(*entry) as u8, 15);
        }

        // New topmost B: marker inserted, increase queue seeded.
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk_b).is_some(),
            "new topmost seeded"
        );
        let b_ws = app
            .world()
            .get::<SkyLightWorkspace>(chunk_b)
            .expect("sky ws on B");
        assert_eq!(b_ws.increase_queue.len(), 256);
    }

    #[test]
    fn seed_initial_light_skips_skyless_dim_for_sky_seed() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, false);
        let palette = spawn_palette_with_torches(&[(2, 2, 2)]);
        // Skyless dim: spawn the chunk without a SkyLight/SkyLightWorkspace
        // (matching the skyless-dimension contract).
        let (chunk, _col) = spawn_topmost_chunk_for_seed(&mut app, dim, palette, false);

        app.update();

        // Block-light emitter seed lands as usual.
        let block_ws = app
            .world()
            .get::<BlockLightWorkspace>(chunk)
            .expect("block ws");
        assert_eq!(block_ws.increase_queue.len(), 1);

        // No sky workspace was attached, so sky pathways are inert.
        assert!(
            app.world().get::<SkyLightWorkspace>(chunk).is_none(),
            "skyless dim has no sky workspace"
        );
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(chunk).is_none(),
            "skyless dim chunk does not insert SkyLightSeededAsTopmost"
        );
        assert!(
            app.world().get::<LightDirty>(chunk).is_some(),
            "chunk still marked LightDirty"
        );
    }

    fn build_pull_neighbor_app() -> App {
        let mut app = App::new();
        app.insert_resource(make_test_table());
        app.add_systems(Update, pull_neighbor_edge_levels);
        app
    }

    #[test]
    fn pull_neighbor_edge_levels_seeds_from_loaded_neighbors() {
        let mut app = build_pull_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        // Two adjacent columns: column_a at x=0, column_b at x=1, both at z=0.
        // Chunk A in column_a at chunk_pos (0,0,0) with BlockLight Uniform(8).
        // Chunk B in column_b at chunk_pos (1,0,0) with BlockLight Null;
        // when B gets Added<ChunkLoaded>, it should pull face cells from A
        // (A is West of B; from B's frame, light enters via the West face).
        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let column_a = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let column_b = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        // Populate dim's ColumnIndex so resolve_loaded_neighbor finds neighbours.
        let mut col_index = app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("column index");
        col_index.0.insert(
            ColumnPos::new(0, 0),
            ColumnSlot {
                entity: column_a,
                section_count: 1,
            },
        );
        col_index.0.insert(
            ColumnPos::new(1, 0),
            ColumnSlot {
                entity: column_b,
                section_count: 1,
            },
        );

        // Chunk A: already loaded, with uniform block light = 8.
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight(crate::storage::LightStorage::Uniform(8)),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
            ChunkLoaded,
        ));

        // Chunk B: just-loaded; Added<ChunkLoaded> fires on its insertion.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        // Drain the existing Added<ChunkLoaded> flag for chunk_a by running
        // one tick first with chunk_b not yet ChunkLoaded; otherwise A would
        // also match the Added filter and start pulling from a non-existent
        // east neighbour (which is fine but obscures the assertion).
        app.update();

        // Now insert ChunkLoaded on chunk_b — that triggers Added on the
        // next app.update() for pull_neighbor_edge_levels.
        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("incoming on B");
        assert_eq!(
            incoming.0.len(),
            256,
            "B pulls 16x16 face cells from A (block-light)"
        );
        // The face direction from B's perspective: A is west of B, so the
        // face is West (index 4); from B's frame, light enters via West.
        let west_index = Direction::West.index() as u8;
        for w in incoming.0.iter() {
            assert_eq!(w.face(), west_index, "face index is West (entry from A)");
            assert_eq!(w.level(), 7, "level = 8 - 1 manhattan attenuation");
        }
        assert!(
            app.world().get::<LightDirty>(chunk_b).is_some(),
            "B marked LightDirty (pulled face cells into its incoming)"
        );
        // The pull from A's face is a non-mutating read on A — A's stored
        // levels and queues did not change. Marking A `LightDirty` here
        // would force a no-op converge pass through it; the natural
        // egress→distribute path re-dirties A automatically if B emits
        // anything toward it during convergence. The pending-egress drain
        // test below covers the case where the neighbour DOES need to be
        // marked dirty (it actually lost a buffered wavefront).
        assert!(
            app.world().get::<LightDirty>(chunk_a).is_none(),
            "neighbour A stays clean — non-mutating face-cell pull is not a state change on A"
        );
    }

    #[test]
    fn pull_neighbor_edge_levels_drains_pending_egress_on_load() {
        let mut app = build_pull_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let column_a = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let column_b = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut col_index = app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("column index");
        col_index.0.insert(
            ColumnPos::new(0, 0),
            ColumnSlot {
                entity: column_a,
                section_count: 1,
            },
        );
        col_index.0.insert(
            ColumnPos::new(1, 0),
            ColumnSlot {
                entity: column_b,
                section_count: 1,
            },
        );

        // A is West of B. From A's frame, the East face (index 5) points
        // toward B. So A's BlockPendingEgress entry with face=East addresses
        // B; the pull system should drain it.
        let east_index = Direction::East.index() as u8;
        let mut pending = BlockPendingEgress::default();
        pending.0.push(Wavefront::new(east_index, 3, 5, 9));

        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight::default(),
            pending,
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
            ChunkLoaded,
        ));

        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        // Tick once to consume the initial Added<ChunkLoaded> on A.
        app.update();

        // Confirm A's pending egress still has the entry (B wasn't loaded
        // during the first tick so the pull system saw no Added<ChunkLoaded>
        // events from B).
        let a_pending_before = app
            .world()
            .get::<BlockPendingEgress>(chunk_a)
            .expect("pending on A");
        assert_eq!(a_pending_before.0.len(), 1, "pending entry survives first tick");

        // Insert ChunkLoaded on chunk_b — Added<ChunkLoaded> fires next tick.
        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        // A's pending egress drained (the East-facing entry moved to B).
        let a_pending_after = app
            .world()
            .get::<BlockPendingEgress>(chunk_a)
            .expect("pending on A");
        assert!(
            a_pending_after.0.is_empty(),
            "A's pending egress drained after B loaded"
        );

        // B's BlockIncoming contains both the face-cell pull entries AND the
        // drained pending entry (face=West in B's frame).
        let b_incoming = app
            .world()
            .get::<BlockIncoming>(chunk_b)
            .expect("incoming on B");
        let west_index = Direction::West.index() as u8;
        let drained = b_incoming
            .0
            .iter()
            .find(|w| w.cell_x() == 3 && w.cell_z() == 5 && w.level() == 9);
        assert!(
            drained.is_some(),
            "drained pending wavefront landed in B's incoming"
        );
        assert_eq!(drained.unwrap().face(), west_index);

        assert!(
            app.world().get::<LightDirty>(chunk_a).is_some(),
            "A marked LightDirty"
        );
    }

    fn build_consume_needs_full_reseed_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, consume_needs_full_reseed);
        app
    }

    #[test]
    fn consume_needs_full_reseed_marks_all_loaded_chunks_when_scan_finalized() {
        let mut app = build_consume_needs_full_reseed_app();

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        let chunk_unloaded_slot: Option<bevy_ecs::entity::Entity> = None;
        // Attach a finalized scan so the system treats the heightmap as
        // primed and proceeds with the reseed.
        let mut scan = crate::lifecycle::ColumnHeightmapScan::new(0, 2);
        scan.scan_cursor = -1;
        assert!(scan.is_finalized());
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), chunk_unloaded_slot, Some(chunk_b)]
                        .into_boxed_slice(),
                },
                scan,
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<ChunkNeedsInitialLight>(chunk_a).is_some(),
            "chunk A re-marked ChunkNeedsInitialLight"
        );
        assert!(
            app.world().get::<ChunkNeedsInitialLight>(chunk_b).is_some(),
            "chunk B re-marked ChunkNeedsInitialLight"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed removed from column"
        );
    }

    /// Regression: when the column's heightmap scan is not yet finalized
    /// (sentinel reads), `consume_needs_full_reseed` must DROP the reseed
    /// instead of re-marking chunks. Re-marking now would cause
    /// `seed_initial_light` to read sentinel `min_y` and misclassify cave
    /// chunks as Case A (Uniform(15)). The natural lifecycle in
    /// `prime_heightmaps_on_column_spawn` inserts the marker once the scan
    /// closes.
    #[test]
    fn consume_needs_full_reseed_drops_reseed_when_scan_not_finalized() {
        let mut app = build_consume_needs_full_reseed_app();

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();
        // No ColumnHeightmapScan attached → unfinalized.
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), None, Some(chunk_b)]
                        .into_boxed_slice(),
                },
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<ChunkNeedsInitialLight>(chunk_a).is_none(),
            "chunk A must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<ChunkNeedsInitialLight>(chunk_b).is_none(),
            "chunk B must NOT be re-marked when scan unfinalized"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed cleared from column"
        );
    }

    /// A scan present but still mid-scan (not finalized) must also drop the
    /// reseed. Same rationale as the no-scan case.
    #[test]
    fn consume_needs_full_reseed_drops_reseed_when_scan_mid_progress() {
        let mut app = build_consume_needs_full_reseed_app();

        let chunk_a = app.world_mut().spawn_empty().id();
        // Mid-scan: cursor still at top of range, no bits closed.
        let scan = crate::lifecycle::ColumnHeightmapScan::new(0, 2);
        assert!(!scan.is_finalized());
        let column = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a), None, None].into_boxed_slice(),
                },
                scan,
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<ChunkNeedsInitialLight>(chunk_a).is_none(),
            "chunk must NOT be re-marked when scan is mid-progress"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed cleared from column"
        );
    }

    #[test]
    fn pull_neighbor_edge_levels_pulls_from_uniform15_on_simultaneous_load() {
        // Regression test for the case where a Case-A (Uniform(15)) neighbour
        // and a dark (Case-B) chunk both receive Added<ChunkLoaded> in the
        // same tick. The old skip at newly_loaded_set skipped the pull
        // unconditionally, so the dark chunk never received the neighbour's
        // level-15 face cells and remained at 0. The fix: only skip when the
        // neighbour is NOT already Uniform(15).
        let mut app = build_pull_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let chunk_a = app.world_mut().spawn_empty().id();
        let chunk_b = app.world_mut().spawn_empty().id();

        let column_a = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_a)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let column_b = app
            .world_mut()
            .spawn((
                Column,
                ColumnChunks {
                    min_section_y: 0,
                    sections: vec![Some(chunk_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut col_index = app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("column index");
        col_index.0.insert(
            ColumnPos::new(0, 0),
            ColumnSlot {
                entity: column_a,
                section_count: 1,
            },
        );
        col_index.0.insert(
            ColumnPos::new(1, 0),
            ColumnSlot {
                entity: column_b,
                section_count: 1,
            },
        );

        // Chunk A: Case A — sky light already at Uniform(15) (set by
        // seed_initial_light before pull_neighbor_edge_levels runs).
        app.world_mut().entity_mut(chunk_a).insert((
            ChunkPos::new(0, 0, 0),
            InColumn(column_a),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            SkyLight(crate::storage::LightStorage::Uniform(15)),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        // Chunk B: Case B — sky light starts at Null (dark), has a
        // SkyIncoming buffer for the pull to write into.
        app.world_mut().entity_mut(chunk_b).insert((
            ChunkPos::new(1, 0, 0),
            InColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        // Insert ChunkLoaded on both in the same tick so both land in
        // newly_loaded_set. The pull system runs once after both inserts.
        app.world_mut().entity_mut(chunk_a).insert(ChunkLoaded);
        app.world_mut().entity_mut(chunk_b).insert(ChunkLoaded);
        app.update();

        let incoming = app
            .world()
            .get::<SkyIncoming>(chunk_b)
            .expect("sky incoming on B");
        assert_eq!(
            incoming.0.len(),
            256,
            "B must receive 256 sky-light face-cell entries from A (16x16 at level 14)"
        );
        let west_index = Direction::West.index() as u8;
        for w in incoming.0.iter() {
            assert_eq!(
                w.face(),
                west_index,
                "all entries enter from the West face (A is West of B)"
            );
            assert_eq!(
                w.level(),
                14,
                "level = 15 - 1 manhattan attenuation"
            );
        }
        assert!(
            app.world().get::<LightDirty>(chunk_b).is_some(),
            "B must be marked LightDirty so the BFS converge loop runs"
        );
    }
}
