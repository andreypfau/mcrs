//! Consumes `MessageReader<BlockPlaced>`, derives the section's intra-cell
//! coord via `rem_euclid(16)` on i32, looks up old/new emission via
//! `Res<BlockLightTable>`, and pushes a decrease and/or increase seed into
//! the section's `BlockLightWorkspace` queues per the emission-diff rule:
//! `old_emission > new_emission` → decrease seed at `old_emission`,
//! `new_emission > 0` → increase seed at `new_emission`. `LightDirty` is
//! inserted via `Commands::entity(placed.chunk).insert(LightDirty)` when at
//! least one seed was pushed.
//!
//! Dampening-only changes (`old_emission == new_emission &&
//! old_dampening != new_dampening`) emit a `tracing::warn!` and skip; the
//! cell will desync until cross-section distribute lands. Missing
//! `BlockLight`/`BlockLightWorkspace` components on the message's
//! `chunk` entity also emit a warning and skip — defensive against any
//! lifecycle-ordering hazard.

use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Added, Commands, Entity, ParamSet, Query, Res, With, Without};
use mcrs_core::voxel_shape::Direction;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::{ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{
    ChunkColumn, ColumnIndex, InChunkColumn, SectionIndex, SectionLookup,
};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;

use crate::bfs::{
    normal_of, pack_bfs_entry, ALL_DIRECTIONS_BITSET, FLAG_RECHECK_LEVEL, FLAG_WRITE_LEVEL,
};
use crate::components::{
    BlockIncoming, BlockLight, BlockLightWorkspace, BlockPendingEgress, ChunkNeedsInitialLight,
    LightDirty, NeedsFullReseed, SkyIncoming, SkyLight, SkyLightSeededAsTopmost, SkyLightWorkspace,
    SkyPendingEgress, Wavefront,
};
use crate::table::{flag_bits, BlockLightTable};

pub fn enqueue_block_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut sections: Query<(&mut BlockLight, &mut BlockLightWorkspace)>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((mut light, mut workspace)) = sections.get_mut(placed.chunk) else {
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
                "dampening-only change not yet handled; light will desync until cross-section distribute lands"
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

/// Seeds the top face of every newly-attached topmost-of-column `ChunkSection`
/// with 256 BFS entries at `(x, z, 15)` level `15` carrying `FLAG_WRITE_LEVEL`.
///
/// "Topmost of column" is decided against the column's `SectionIndex`:
/// `chunk_pos.y == min_section_y + sections.len() - 1`. Non-topmost sections
/// (lower in the column) seed nothing — sky light reaches them only via the
/// downward BFS step from the section above. Sections in skyless dimensions
/// never receive a `SkyLight` component (the `SkyLightBundle` insertion gate
/// in `lifecycle::attach_lighting_state` keys on `HasSkyLight`), so the
/// `Added<SkyLight>` filter self-gates this system.
pub fn enqueue_sky_light_initial(
    mut newly_added: Query<
        (Entity, &ChunkPos, &InChunkColumn, &mut SkyLightWorkspace),
        Added<SkyLight>,
    >,
    columns: Query<&SectionIndex>,
    mut commands: Commands,
) {
    for (section_entity, chunk_pos, in_column, mut workspace) in newly_added.iter_mut() {
        let Ok(section_index) = columns.get(in_column.0) else {
            continue;
        };
        let top_chunk_y =
            section_index.min_section_y + section_index.sections.len() as i32 - 1;
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
        commands.entity(section_entity).insert(LightDirty);
    }
}

/// Reacts to `BlockPlaced` by enqueuing sky-light decrease and increase seeds
/// whenever the placed block changes either its dampening or its
/// `PROPAGATES_SKYLIGHT_DOWN` flag. The system also records the world Y of the
/// change into the per-column `block_change_tracker` so the downstream
/// heightmap-update pass can decide whether the column's surface dropped.
///
/// Missing `SkyLight`/`SkyLightWorkspace` components on the target section
/// emit a `tracing::warn!` and skip without panic; this defends against
/// `BlockPlaced` reaching a skyless-dim section (where the bundle is never
/// attached) or arriving before the lighting bundle insertion has flushed.
pub fn enqueue_sky_light_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    table: Res<BlockLightTable>,
    mut sections: Query<(
        &mut SkyLight,
        &mut SkyLightWorkspace,
        &ChunkPos,
        &InChunkColumn,
    )>,
    columns: Query<&SectionIndex>,
    mut commands: Commands,
) {
    for placed in reader.read() {
        if placed.old_state == placed.new_state {
            continue;
        }

        let Ok((mut light, mut workspace, chunk_pos, in_column)) =
            sections.get_mut(placed.chunk)
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
            Ok(section_index) => {
                let top_chunk_y =
                    section_index.min_section_y + section_index.sections.len() as i32 - 1;
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

        let column_idx = (z as usize) * 16 + (x as usize);
        let world_y = placed.block_pos.y;
        if world_y > workspace.block_change_tracker[column_idx] {
            workspace.block_change_tracker[column_idx] = world_y;
        }

        commands.entity(placed.chunk).insert(LightDirty);
    }
}

/// Returns the (x, y, z) coordinates of the neighbour-section face cell that
/// adjoins us on `from_face` (the face direction in the NEIGHBOUR's frame, i.e.
/// `face.opposite()` of the face from us to the neighbour). `(cell_a, cell_b)`
/// are the on-face coordinates; the normal axis is set to the appropriate
/// boundary value (0 or 15).
#[inline]
fn face_cell_coords_in_neighbor_frame(from_face: Direction, cell_a: u8, cell_b: u8) -> (u8, u8, u8) {
    match from_face {
        Direction::Down => (cell_a, 0, cell_b),
        Direction::Up => (cell_a, 15, cell_b),
        Direction::North => (cell_a, cell_b, 0),
        Direction::South => (cell_a, cell_b, 15),
        Direction::West => (0, cell_a, cell_b),
        Direction::East => (15, cell_a, cell_b),
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

/// Resolves a neighbour section by walking through `SectionIndex` (for Y-axis
/// neighbours) or `ColumnIndex` + `SectionIndex` (for X/Z-axis neighbours).
/// Returns `Some(entity)` only for `SectionLookup::Loaded(entity)`; padding and
/// out-of-range and unloaded neighbours all return `None`.
fn resolve_loaded_neighbor(
    face: Direction,
    chunk_pos: ChunkPos,
    in_col: Entity,
    in_dim: Entity,
    column_indexes: &Query<&ColumnIndex>,
    section_indexes: &Query<&SectionIndex>,
) -> Option<Entity> {
    match face {
        Direction::Up | Direction::Down => {
            let section_index = section_indexes.get(in_col).ok()?;
            let dy = if face == Direction::Up { 1 } else { -1 };
            match section_index.lookup(chunk_pos.y + dy) {
                SectionLookup::Loaded(e) => Some(e),
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
                mcrs_engine::world::column::ChunkColumnPos::new(nx, nz);
            let slot = column_index.0.get(&neighbour_col_pos)?;
            let neighbour_section_index = section_indexes.get(slot.entity).ok()?;
            match neighbour_section_index.lookup(chunk_pos.y) {
                SectionLookup::Loaded(e) => Some(e),
                _ => None,
            }
        }
    }
}

/// Consumes `ChunkNeedsInitialLight` per section: scans the palette for
/// block-light emitters and seeds `BlockLightWorkspace::increase_queue`; on a
/// sky-having dimension's topmost section, additionally seeds 256
/// `SkyLightWorkspace::increase_queue` entries at y=15. On retopping, also
/// drives a decrease wave through the previously-topmost section.
///
/// The query is gated on `Option<Res<BlockLightTable>>` for consistency with
/// `prime_heightmaps_on_column_spawn` — early-returns if the resource has not
/// been built yet (registry freeze races early ticks).
pub fn seed_initial_light(
    table: Option<Res<BlockLightTable>>,
    sky_dims: Query<(), With<HasSkyLight>>,
    section_indexes: Query<&SectionIndex>,
    mut sections: ParamSet<(
        Query<
            (
                Entity,
                &BlockPalette,
                &InChunkColumn,
                &InDimension,
                &ChunkPos,
                &mut BlockLightWorkspace,
                Option<&mut SkyLightWorkspace>,
            ),
            With<ChunkNeedsInitialLight>,
        >,
        Query<
            (Entity, &ChunkPos, &InChunkColumn, &mut SkyLightWorkspace, &SkyLight),
            With<SkyLightSeededAsTopmost>,
        >,
    )>,
    mut commands: Commands,
) {
    let Some(table) = table else {
        return;
    };

    // First pass: collect what needs to happen, since we can't hold p0() and
    // p1() borrows simultaneously. For each section in p0(), determine block
    // emitters, sky seeding, and a "previously-topmost invalidate" target.
    struct Plan {
        section: Entity,
        column: Entity,
        seeded_topmost: bool,
        new_chunk_y: i32,
    }
    let mut plans: Vec<Plan> = Vec::new();

    {
        let mut p0 = sections.p0();
        for (
            section_entity,
            palette,
            in_col,
            in_dim,
            chunk_pos,
            mut block_ws,
            mut sky_ws_opt,
        ) in p0.iter_mut()
        {
            // Block-light emitter scan. Always run the cell-by-cell scan: the
            // for_each_distinct_state check would only skip the 4096-cell loop
            // for sections with zero emitters, which is the common case, but
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

            // Sky source seed when the section is topmost-of-column AND the
            // dimension carries HasSkyLight AND a SkyLightWorkspace is present
            // (skyless dims do not receive a SkyLightBundle).
            let dim_has_sky = sky_dims.get(in_dim.0).is_ok();
            let is_topmost = section_indexes
                .get(in_col.0)
                .ok()
                .map(|si| chunk_pos.y == si.min_section_y + si.sections.len() as i32 - 1)
                .unwrap_or(false);

            let mut seeded = false;
            if dim_has_sky && is_topmost {
                if let Some(sky_ws) = sky_ws_opt.as_deref_mut() {
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
                        .entity(section_entity)
                        .insert(SkyLightSeededAsTopmost);
                    seeded = true;
                }
            }

            commands.entity(section_entity).insert(LightDirty);
            commands
                .entity(section_entity)
                .remove::<ChunkNeedsInitialLight>();

            plans.push(Plan {
                section: section_entity,
                column: in_col.0,
                seeded_topmost: seeded,
                new_chunk_y: chunk_pos.y,
            });
        }
    }

    // Second pass: for each plan that seeded a new topmost, find any previously-
    // topmost section in the SAME column with a lower chunk_pos.y and walk a
    // decrease wave through its top face using the stored sky levels. The pass
    // owns the &mut SkyLightWorkspace on the previous-topmost entity here
    // exclusively, since the first pass already released its borrow.
    if plans.iter().any(|p| p.seeded_topmost) {
        let mut p1 = sections.p1();
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

/// Consumes `Added<ChunkLoaded>` per section: reads each loaded cardinal
/// neighbour's face cells into the new section's `*Incoming`, then drains any
/// `*PendingEgress` entries that the neighbour buffered while we were
/// unloaded. Marks the new section and every touched loaded neighbour
/// `LightDirty`.
pub fn pull_neighbor_edge_levels(
    table: Option<Res<BlockLightTable>>,
    newly_loaded: Query<(Entity, &ChunkPos, &InDimension, &InChunkColumn), Added<ChunkLoaded>>,
    column_indexes: Query<&ColumnIndex>,
    section_indexes: Query<&SectionIndex>,
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

    for (new_section, chunk_pos, in_dim, in_col) in newly_loaded.iter() {
        for face in CARDINAL_DIRECTIONS {
            let Some(neighbour_entity) = resolve_loaded_neighbor(
                face,
                *chunk_pos,
                in_col.0,
                in_dim.0,
                &column_indexes,
                &section_indexes,
            ) else {
                continue;
            };

            // `face` is the direction from us (new section) to the neighbour
            // in OUR (destination) frame, so it doubles as the incoming face
            // index. `from_face` is the neighbour's frame face pointing back
            // at us; we use it both to compute the neighbour's face-cell
            // coordinates and to filter the neighbour's pending-egress entries
            // (which are tagged in the neighbour's frame).
            let from_face = face.opposite();
            let dest_face = face.index() as u8;
            let neighbour_expected_face = from_face.index() as u8;

            // Read neighbour's face cells into incoming with Manhattan-1
            // pre-attenuation.
            for cell_a in 0..16u8 {
                for cell_b in 0..16u8 {
                    let (nx, ny, nz) =
                        face_cell_coords_in_neighbor_frame(from_face, cell_a, cell_b);

                    if let Ok(bl) = block_light_read.get(neighbour_entity) {
                        let level = bl.0.get(nx as usize, ny as usize, nz as usize);
                        if level > 0 {
                            let attenuated = level.saturating_sub(1);
                            if let Ok(mut inc) = block_incoming.get_mut(new_section) {
                                inc.0.push(Wavefront::new(
                                    dest_face, cell_a, cell_b, attenuated,
                                ));
                            }
                        }
                    }

                    if let Ok(sl) = sky_light_read.get(neighbour_entity) {
                        let level = sl.0.get(nx as usize, ny as usize, nz as usize);
                        if level > 0 {
                            let attenuated = level.saturating_sub(1);
                            if let Ok(mut inc) = sky_incoming.get_mut(new_section) {
                                inc.0.push(Wavefront::new(
                                    dest_face, cell_a, cell_b, attenuated,
                                ));
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
                            if let Ok(mut inc) = block_incoming.get_mut(new_section) {
                                inc.0.push(Wavefront::new(
                                    dest_face,
                                    w.cell_x(),
                                    w.cell_z(),
                                    w.level(),
                                ));
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
                            if let Ok(mut inc) = sky_incoming.get_mut(new_section) {
                                inc.0.push(Wavefront::new(
                                    dest_face,
                                    w.cell_x(),
                                    w.cell_z(),
                                    w.level(),
                                ));
                            }
                            false
                        } else {
                            true
                        }
                    });
                }
            }

            commands.entity(neighbour_entity).insert(LightDirty);
        }

        // Mark the just-loaded section LightDirty so the first convergence
        // iteration drains its *Incoming. We unconditionally insert: even
        // when there are no loaded neighbours, the section still went through
        // seed_initial_light and needs to be considered dirty.
        commands.entity(new_section).insert(LightDirty);
    }
}

/// Consumes `Added<NeedsFullReseed>` on `ChunkColumn` entities: iterates the
/// column's `SectionIndex.sections` slots and re-inserts
/// `ChunkNeedsInitialLight` on every loaded section in the column. Removes
/// `NeedsFullReseed` from the column entity.
pub fn consume_needs_full_reseed(
    newly_marked: Query<(Entity, &SectionIndex), (With<ChunkColumn>, Added<NeedsFullReseed>)>,
    mut commands: Commands,
) {
    for (column_entity, section_index) in newly_marked.iter() {
        for slot in section_index.sections.iter() {
            if let Some(section_entity) = slot {
                commands
                    .entity(*section_entity)
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
    use mcrs_engine::world::column::{ChunkColumn, ChunkColumnPos, ColumnIndex, ColumnSlot, InChunkColumn, SectionIndex};
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

    fn spawn_section(app: &mut App) -> bevy_ecs::entity::Entity {
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
        let entity = spawn_section(&mut app);
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
        let entity = spawn_section(&mut app);
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
        let entity = spawn_section(&mut app);
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
        let entity = spawn_section(&mut app);
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
        let entity = spawn_section(&mut app);
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
        // a section the lighting lifecycle has not yet attached state to.
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
        let entity = spawn_section(&mut app);
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

    fn spawn_column_with_sections(
        app: &mut App,
        min_section_y: i32,
        section_slots: Vec<Option<bevy_ecs::entity::Entity>>,
    ) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn(SectionIndex {
                min_section_y,
                sections: section_slots.into_boxed_slice(),
            })
            .id()
    }

    #[test]
    fn enqueue_sky_initial_seeds_topmost_section_only() {
        let mut app = build_sky_initial_app();

        let section = app.world_mut().spawn_empty().id();
        let column = spawn_column_with_sections(&mut app, 0, vec![Some(section)]);
        app.world_mut().entity_mut(section).insert((
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(section)
            .expect("sky workspace");
        assert_eq!(
            workspace.increase_queue.len(),
            256,
            "topmost-of-column section seeds 256 entries (16 x 16 at y=15)"
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
            app.world().get::<LightDirty>(section).is_some(),
            "LightDirty inserted on topmost-of-column seed"
        );
    }

    #[test]
    fn enqueue_sky_initial_skips_non_topmost() {
        let mut app = build_sky_initial_app();

        let section_below = app.world_mut().spawn_empty().id();
        let section_topmost = app.world_mut().spawn_empty().id();
        // Two-section column: chunk-Y 0 (below) and chunk-Y 1 (topmost).
        let column = spawn_column_with_sections(
            &mut app,
            0,
            vec![Some(section_below), Some(section_topmost)],
        );
        // Only the below section gets SkyLight added; topmost is left bare
        // so this single test does not also seed an unrelated section.
        app.world_mut().entity_mut(section_below).insert((
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
            SkyLight::default(),
            SkyLightWorkspace::default(),
        ));

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(section_below)
            .expect("sky workspace");
        assert!(
            workspace.increase_queue.is_empty(),
            "non-topmost section seeds nothing"
        );
        assert!(
            workspace.decrease_queue.is_empty(),
            "non-topmost section seeds no decrease"
        );
        assert!(
            app.world().get::<LightDirty>(section_below).is_none(),
            "LightDirty NOT inserted on non-topmost-of-column section"
        );
    }

    fn build_sky_on_placed_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, enqueue_sky_light_on_block_placed);
        app
    }

    fn spawn_sky_section_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let section = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(SectionIndex {
                min_section_y: 0,
                sections: vec![Some(section)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(section).insert((
            SkyLight::default(),
            SkyLightWorkspace::default(),
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
        ));
        section
    }

    fn spawn_sky_section_non_topmost(app: &mut App) -> bevy_ecs::entity::Entity {
        let section = app.world_mut().spawn_empty().id();
        let dummy_topmost = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn(SectionIndex {
                min_section_y: 0,
                sections: vec![Some(section), Some(dummy_topmost)].into_boxed_slice(),
            })
            .id();
        app.world_mut().entity_mut(section).insert((
            SkyLight::default(),
            SkyLightWorkspace::default(),
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
        ));
        section
    }

    #[test]
    fn enqueue_sky_on_block_placed_writes_tracker() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section_topmost(&mut app);
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
        // column_idx = z * 16 + x = 8 * 16 + 8 = 136; world_y = 10.
        assert_eq!(
            workspace.block_change_tracker[136], 10,
            "block_change_tracker[z*16+x] holds the world Y of the change"
        );
        assert!(
            !workspace.decrease_queue.is_empty(),
            "dampening change pushes a decrease seed"
        );
        assert!(
            !workspace.increase_queue.is_empty(),
            "y=10 (non-top) pushes neighbour-support increase seeds"
        );
        // y=10 (intra-section, not 15) -> six neighbour seeds.
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
    fn enqueue_sky_on_block_placed_writes_tracker_keeps_max() {
        // Two BlockPlaced events at the same (x, z) column; tracker must keep
        // the larger world Y to preserve the highest changed cell.
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 12, 8), AIR, LEAVES),
        );
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(8, 5, 8), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        assert_eq!(
            workspace.block_change_tracker[136], 12,
            "tracker keeps max(existing, world_y); 12 > 5 wins"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_top_seeds_top_face() {
        // y == 15 path: a single top-face increase seed instead of six
        // neighbour seeds.
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section_topmost(&mut app);
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
            "top-of-section seed carries FLAG_WRITE_LEVEL"
        );
    }

    #[test]
    fn enqueue_sky_on_block_placed_skips_when_predicate_false() {
        let mut app = build_sky_on_placed_app();
        let entity = spawn_sky_section_topmost(&mut app);
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
            workspace.block_change_tracker.iter().all(|&v| v == 0),
            "tracker untouched when predicate is false"
        );
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
            // Section without SkyLight/SkyLightWorkspace (skyless-dim shape).
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
        let entity = spawn_sky_section_topmost(&mut app);
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
        let entity = spawn_sky_section_topmost(&mut app);
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
        let entity = spawn_sky_section_non_topmost(&mut app);
        write_placed(
            &mut app,
            block_placed(entity, BlockPos::new(3, 15, 9), AIR, LEAVES),
        );

        app.update();

        let workspace = app
            .world()
            .get::<SkyLightWorkspace>(entity)
            .expect("sky workspace");
        // y=15 sits at the top of the section, so the Up neighbour at y=16
        // is outside the section and is skipped by the bounds guard. Five
        // neighbour-recheck seeds remain.
        assert_eq!(
            workspace.increase_queue.len(),
            5,
            "non-topmost section falls through to neighbour-recheck branch at y=15"
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
        let entity = spawn_sky_section_topmost(&mut app);
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
            "topmost section emits a single top-face seed at y=15"
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
        let entity = spawn_sky_section_topmost(&mut app);
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

    fn spawn_topmost_section_for_seed(
        app: &mut App,
        dim: bevy_ecs::entity::Entity,
        palette: BlockPalette,
        sky: bool,
    ) -> (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity) {
        let section = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                ChunkColumn,
                SectionIndex {
                    min_section_y: 0,
                    sections: vec![Some(section)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let mut emut = app.world_mut().entity_mut(section);
        emut.insert((
            palette,
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            ChunkNeedsInitialLight,
        ));
        if sky {
            emut.insert((SkyLight::default(), SkyLightWorkspace::default()));
        }
        (section, column)
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
        let (section, _col) = spawn_topmost_section_for_seed(&mut app, dim, palette, true);

        app.update();

        let block_ws = app
            .world()
            .get::<BlockLightWorkspace>(section)
            .expect("block ws");
        assert_eq!(
            block_ws.increase_queue.len(),
            5,
            "five torches emit five increase seeds"
        );
        let sky_ws = app
            .world()
            .get::<SkyLightWorkspace>(section)
            .expect("sky ws");
        assert_eq!(
            sky_ws.increase_queue.len(),
            256,
            "topmost on sky-having dim seeds 256 sky entries"
        );
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(section).is_some(),
            "SkyLightSeededAsTopmost inserted"
        );
        assert!(
            app.world().get::<LightDirty>(section).is_some(),
            "LightDirty inserted"
        );
        assert!(
            app.world().get::<ChunkNeedsInitialLight>(section).is_none(),
            "ChunkNeedsInitialLight removed"
        );
    }

    #[test]
    fn seed_initial_light_invalidates_previous_topmost_on_retopping() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, true);

        // Section A at chunk-Y 0 with SkyLightSeededAsTopmost already; stored
        // sky level 12 across the top face.
        let section_a = app.world_mut().spawn_empty().id();
        let section_b = app.world_mut().spawn_empty().id();
        let column = app
            .world_mut()
            .spawn((
                ChunkColumn,
                SectionIndex {
                    min_section_y: 0,
                    sections: vec![Some(section_a), Some(section_b)].into_boxed_slice(),
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
        app.world_mut().entity_mut(section_a).insert((
            palette_a,
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column),
            InDimension(dim),
            BlockLight::default(),
            BlockLightWorkspace::default(),
            a_sky_light,
            SkyLightWorkspace::default(),
            SkyLightSeededAsTopmost,
        ));

        // Section B at chunk-Y 1 (the new topmost) needs initial light.
        let mut palette_b = BlockPalette::default();
        palette_b.fill(AIR);
        app.world_mut().entity_mut(section_b).insert((
            palette_b,
            ChunkPos::new(0, 1, 0),
            InChunkColumn(column),
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
            app.world().get::<SkyLightSeededAsTopmost>(section_a).is_none(),
            "previous topmost's SkyLightSeededAsTopmost removed"
        );
        assert!(
            app.world().get::<LightDirty>(section_a).is_some(),
            "previous topmost marked LightDirty"
        );
        let a_ws = app
            .world()
            .get::<SkyLightWorkspace>(section_a)
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
            app.world().get::<SkyLightSeededAsTopmost>(section_b).is_some(),
            "new topmost seeded"
        );
        let b_ws = app
            .world()
            .get::<SkyLightWorkspace>(section_b)
            .expect("sky ws on B");
        assert_eq!(b_ws.increase_queue.len(), 256);
    }

    #[test]
    fn seed_initial_light_skips_skyless_dim_for_sky_seed() {
        let mut app = build_seed_initial_app();
        let dim = spawn_dimension(&mut app, false);
        let palette = spawn_palette_with_torches(&[(2, 2, 2)]);
        // Skyless dim: spawn the section without a SkyLight/SkyLightWorkspace
        // (matching the skyless-dimension contract).
        let (section, _col) = spawn_topmost_section_for_seed(&mut app, dim, palette, false);

        app.update();

        // Block-light emitter seed lands as usual.
        let block_ws = app
            .world()
            .get::<BlockLightWorkspace>(section)
            .expect("block ws");
        assert_eq!(block_ws.increase_queue.len(), 1);

        // No sky workspace was attached, so sky pathways are inert.
        assert!(
            app.world().get::<SkyLightWorkspace>(section).is_none(),
            "skyless dim has no sky workspace"
        );
        assert!(
            app.world().get::<SkyLightSeededAsTopmost>(section).is_none(),
            "skyless dim section does not insert SkyLightSeededAsTopmost"
        );
        assert!(
            app.world().get::<LightDirty>(section).is_some(),
            "section still marked LightDirty"
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
        // Section A in column_a at chunk_pos (0,0,0) with BlockLight Uniform(8).
        // Section B in column_b at chunk_pos (1,0,0) with BlockLight Null;
        // when B gets Added<ChunkLoaded>, it should pull face cells from A
        // (A is West of B; from B's frame, light enters via the West face).
        let section_a = app.world_mut().spawn_empty().id();
        let section_b = app.world_mut().spawn_empty().id();
        let column_a = app
            .world_mut()
            .spawn((
                ChunkColumn,
                SectionIndex {
                    min_section_y: 0,
                    sections: vec![Some(section_a)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let column_b = app
            .world_mut()
            .spawn((
                ChunkColumn,
                SectionIndex {
                    min_section_y: 0,
                    sections: vec![Some(section_b)].into_boxed_slice(),
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
            ChunkColumnPos::new(0, 0),
            ColumnSlot {
                entity: column_a,
                section_count: 1,
            },
        );
        col_index.0.insert(
            ChunkColumnPos::new(1, 0),
            ColumnSlot {
                entity: column_b,
                section_count: 1,
            },
        );

        // Section A: already loaded, with uniform block light = 8.
        app.world_mut().entity_mut(section_a).insert((
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column_a),
            InDimension(dim),
            BlockLight(crate::storage::LightStorage::Uniform(8)),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
            ChunkLoaded,
        ));

        // Section B: just-loaded; Added<ChunkLoaded> fires on its insertion.
        app.world_mut().entity_mut(section_b).insert((
            ChunkPos::new(1, 0, 0),
            InChunkColumn(column_b),
            InDimension(dim),
            BlockLight::default(),
            BlockPendingEgress::default(),
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
        ));

        // Drain the existing Added<ChunkLoaded> flag for section_a by running
        // one tick first with section_b not yet ChunkLoaded; otherwise A would
        // also match the Added filter and start pulling from a non-existent
        // east neighbour (which is fine but obscures the assertion).
        app.update();

        // Now insert ChunkLoaded on section_b — that triggers Added on the
        // next app.update() for pull_neighbor_edge_levels.
        app.world_mut().entity_mut(section_b).insert(ChunkLoaded);
        app.update();

        let incoming = app
            .world()
            .get::<BlockIncoming>(section_b)
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
            app.world().get::<LightDirty>(section_b).is_some(),
            "B marked LightDirty"
        );
        assert!(
            app.world().get::<LightDirty>(section_a).is_some(),
            "neighbour A marked LightDirty"
        );
    }

    #[test]
    fn pull_neighbor_edge_levels_drains_pending_egress_on_load() {
        let mut app = build_pull_neighbor_app();
        let dim = spawn_dimension(&mut app, true);

        let section_a = app.world_mut().spawn_empty().id();
        let section_b = app.world_mut().spawn_empty().id();
        let column_a = app
            .world_mut()
            .spawn((
                ChunkColumn,
                SectionIndex {
                    min_section_y: 0,
                    sections: vec![Some(section_a)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();
        let column_b = app
            .world_mut()
            .spawn((
                ChunkColumn,
                SectionIndex {
                    min_section_y: 0,
                    sections: vec![Some(section_b)].into_boxed_slice(),
                },
                InDimension(dim),
            ))
            .id();

        let mut col_index = app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("column index");
        col_index.0.insert(
            ChunkColumnPos::new(0, 0),
            ColumnSlot {
                entity: column_a,
                section_count: 1,
            },
        );
        col_index.0.insert(
            ChunkColumnPos::new(1, 0),
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

        app.world_mut().entity_mut(section_a).insert((
            ChunkPos::new(0, 0, 0),
            InChunkColumn(column_a),
            InDimension(dim),
            BlockLight::default(),
            pending,
            BlockIncoming::default(),
            SkyLight::default(),
            SkyPendingEgress::default(),
            SkyIncoming::default(),
            ChunkLoaded,
        ));

        app.world_mut().entity_mut(section_b).insert((
            ChunkPos::new(1, 0, 0),
            InChunkColumn(column_b),
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
            .get::<BlockPendingEgress>(section_a)
            .expect("pending on A");
        assert_eq!(a_pending_before.0.len(), 1, "pending entry survives first tick");

        // Insert ChunkLoaded on section_b — Added<ChunkLoaded> fires next tick.
        app.world_mut().entity_mut(section_b).insert(ChunkLoaded);
        app.update();

        // A's pending egress drained (the East-facing entry moved to B).
        let a_pending_after = app
            .world()
            .get::<BlockPendingEgress>(section_a)
            .expect("pending on A");
        assert!(
            a_pending_after.0.is_empty(),
            "A's pending egress drained after B loaded"
        );

        // B's BlockIncoming contains both the face-cell pull entries AND the
        // drained pending entry (face=West in B's frame).
        let b_incoming = app
            .world()
            .get::<BlockIncoming>(section_b)
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
            app.world().get::<LightDirty>(section_a).is_some(),
            "A marked LightDirty"
        );
    }

    fn build_consume_needs_full_reseed_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, consume_needs_full_reseed);
        app
    }

    #[test]
    fn consume_needs_full_reseed_marks_all_loaded_sections() {
        let mut app = build_consume_needs_full_reseed_app();

        let section_a = app.world_mut().spawn_empty().id();
        let section_b = app.world_mut().spawn_empty().id();
        let section_unloaded_slot: Option<bevy_ecs::entity::Entity> = None;
        let column = app
            .world_mut()
            .spawn((
                ChunkColumn,
                SectionIndex {
                    min_section_y: 0,
                    sections: vec![Some(section_a), section_unloaded_slot, Some(section_b)]
                        .into_boxed_slice(),
                },
            ))
            .id();
        app.world_mut().entity_mut(column).insert(NeedsFullReseed);

        app.update();

        assert!(
            app.world().get::<ChunkNeedsInitialLight>(section_a).is_some(),
            "section A re-marked ChunkNeedsInitialLight"
        );
        assert!(
            app.world().get::<ChunkNeedsInitialLight>(section_b).is_some(),
            "section B re-marked ChunkNeedsInitialLight"
        );
        assert!(
            app.world().get::<NeedsFullReseed>(column).is_none(),
            "NeedsFullReseed removed from column"
        );
    }
}
