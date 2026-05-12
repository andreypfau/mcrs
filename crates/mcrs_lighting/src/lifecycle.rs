// Heightmaps zero-init convention: `Heightmaps::new(height)` and
// `Heightmaps::with_min_y(height, min_y)` zero-initialize the backing
// `PackedBitStorage` long arrays, so `surface_get(x, z) == min_y` for any
// unprimed (x, z) until this system overwrites with the real top-down-scan
// result. Downstream eager-update code MUST use `min_y` as the
// "no surface found" sentinel to stay deterministic.

use crate::bundle::{BlockLightBundle, SkyLightBundle};
use crate::components::{ChunkNeedsInitialLight, IsAllAir};
use crate::table::{flag_bits, BlockLightTable};
use bevy_ecs::prelude::{Added, Commands, Entity, Query, Res, With};
use mcrs_engine::world::chunk::{ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{ChunkColumn, Heightmaps, SectionIndex, SectionLookup};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_minecraft::world::palette::BlockPalette;

const SECTION_SIZE: i32 = 16;

/// Stage 2.5 of the chunk-column lifecycle. For every newly-spawned column,
/// scan its sections from the top down and prime both `Heightmaps` storages
/// from `BlockLightTable.flags`. Cells with `IS_NOT_AIR` populate
/// `world_surface`; cells with `IS_MOTION_BLOCKING` populate `motion_blocking`.
///
/// Pitfall #1 safety: this system lives in `mcrs_lighting` because it consumes
/// `BlockLightTable`. The engine crate stays free of any lighting-side imports.
///
/// Runs after `ChunkColumnLifecycleSet::ReconcileIndex` (Stage 2) so the
/// column's `SectionIndex` is fully populated for the sections that triggered
/// the column spawn. The leading `ApplyDeferred` in `LightingPlugin::build`
/// flushes Stage 1 + 2's commands so this query observes the spawned column
/// and the inserted section back-link.
///
/// In production `BlockLightTable` is always present because
/// `build_block_light_table` runs strictly before any chunk-column spawn
/// (on `OnEnter(AppState::WorldgenFreeze)`). Integration tests must insert
/// a stub `BlockLightTable` resource before spawning sections.
pub fn prime_heightmaps_on_column_spawn(
    newly_spawned: Query<(Entity, &SectionIndex), (With<ChunkColumn>, Added<ChunkColumn>)>,
    sections: Query<&BlockPalette>,
    mut heightmaps: Query<&mut Heightmaps>,
    table: Res<BlockLightTable>,
) {
    for (column_entity, section_index) in newly_spawned.iter() {
        let Ok(mut hm) = heightmaps.get_mut(column_entity) else {
            continue;
        };
        let min_y = hm.min_y();
        let min_section_y = section_index.min_section_y;

        let mut world_surface_done = [false; 256];
        let mut motion_blocking_done = [false; 256];
        let mut remaining_world_surface: u32 = 256;
        let mut remaining_motion_blocking: u32 = 256;

        for (rel_y, slot) in section_index.sections.iter().enumerate().rev() {
            if remaining_world_surface == 0 && remaining_motion_blocking == 0 {
                break;
            }
            let Some(section_entity) = slot else {
                continue;
            };
            let Ok(palette) = sections.get(*section_entity) else {
                continue;
            };

            let section_base_y =
                (min_section_y + rel_y as i32) * SECTION_SIZE;

            for cell_y in (0..SECTION_SIZE).rev() {
                if remaining_world_surface == 0 && remaining_motion_blocking == 0 {
                    break;
                }
                let world_y = section_base_y + cell_y;
                for z in 0..16usize {
                    for x in 0..16usize {
                        let idx = z * 16 + x;
                        if world_surface_done[idx] && motion_blocking_done[idx] {
                            continue;
                        }
                        let state = palette.get((
                            x as i32,
                            cell_y,
                            z as i32,
                        ));
                        let flags = table.flags_for(state);
                        if !world_surface_done[idx]
                            && (flags & flag_bits::IS_NOT_AIR) != 0
                        {
                            // Vanilla stores "first available" Y above the surface,
                            // i.e. the Y of the empty cell on top of the topmost
                            // solid. `surface_set` clamps to dimension bounds.
                            hm.surface_set(x, z, world_y + 1);
                            world_surface_done[idx] = true;
                            remaining_world_surface -= 1;
                        }
                        if !motion_blocking_done[idx]
                            && (flags & flag_bits::IS_MOTION_BLOCKING) != 0
                        {
                            hm.motion_blocking_set(x, z, world_y + 1);
                            motion_blocking_done[idx] = true;
                            remaining_motion_blocking -= 1;
                        }
                    }
                }
            }
        }

        let _ = min_y;
    }
}

/// Stage 3 of the chunk-column lifecycle. For every newly-loaded
/// `ChunkSection` (sections with `Added<ChunkLoaded>`), attach the per-section
/// lighting bundles and the initial-light seed marker.
///
/// `BlockLightBundle` is inserted unconditionally. `SkyLightBundle` is
/// inserted only when the parent `Dimension` carries `HasSkyLight`.
/// `ChunkNeedsInitialLight` is inserted unconditionally — even skyless
/// dimensions need an initial-light seed pass for block-light emitters.
/// `IsAllAir` is inserted when the section's `BlockPalette` contains only
/// air-equivalent states (`emission == 0 && dampening == 0`).
pub fn attach_lighting_state(
    newly_loaded: Query<(Entity, &BlockPalette, &InDimension, &ChunkPos), Added<ChunkLoaded>>,
    sky_dims: Query<(), With<HasSkyLight>>,
    table: Res<BlockLightTable>,
    mut commands: Commands,
) {
    for (section_entity, palette, in_dim, _chunk_pos) in newly_loaded.iter() {
        let mut entity_commands = commands.entity(section_entity);
        entity_commands.insert(BlockLightBundle::default());
        if sky_dims.get(in_dim.0).is_ok() {
            entity_commands.insert(SkyLightBundle::default());
        }
        entity_commands.insert(ChunkNeedsInitialLight);

        if is_section_all_air(palette, &table) {
            entity_commands.insert(IsAllAir);
        }
    }
}

/// `true` if every cell in the section's palette has `emission == 0` and
/// `dampening == 0`. Uses `BlockPalette::for_each_distinct_state` to avoid
/// scanning all 4096 cells when the palette holds only a handful of states.
fn is_section_all_air(palette: &BlockPalette, table: &BlockLightTable) -> bool {
    let mut all_air = true;
    palette.for_each_distinct_state(|state| {
        if !all_air {
            return;
        }
        let idx = state.0 as usize;
        if idx >= table.len()
            || table.emission[idx] != 0
            || table.dampening[idx] != 0
        {
            all_air = false;
        }
    });
    all_air
}
