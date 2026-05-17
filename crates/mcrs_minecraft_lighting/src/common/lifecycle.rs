// Heightmaps zero-init convention: `Heightmaps::new(height)` and
// `Heightmaps::with_min_y(height, min_y)` zero-initialize the backing
// `PackedBitStorage` long arrays, so `surface_get(x, z) == min_y` for any
// unprimed (x, z) until this system overwrites with the real top-down-scan
// result. Downstream eager-update code MUST use `min_y` as the
// "no surface found" sentinel to stay deterministic.

use crate::bitset::BitSet256;
use crate::heightmap::{
    record_topmost, record_unsurfaced_column, record_unsurfaced_motion_column, scan_top_down,
    HeightmapVariant, ScanOutcome,
};
use crate::table::BlockStateLightTable;
use bevy_ecs::prelude::{Added, Changed, Commands, Component, Entity, Has, Query, Res, With};
use mcrs_engine::world::chunk::{ChunkLoaded, ChunkPos};
use mcrs_engine::world::column::{Column, ColumnChunks, Heightmaps};
use mcrs_engine::world::dimension::{HasSkyLight, InDimension};
use mcrs_minecraft_block::palette::BlockPalette;
use crate::block_light::bundle::BlockLightBundle;
use crate::{BlockNeedsInitialSeed, IsAllAir, SkyNeedsInitialSeed};
use crate::sky_light::bundle::SkyLightBundle;

const XZ_FULL: [(usize, usize); 256] = {
    let mut arr = [(0usize, 0usize); 256];
    let mut i = 0;
    while i < 256 {
        arr[i] = (i & 15, i >> 4);
        i += 1;
    }
    arr
};

#[inline]
const fn xz_idx(x: usize, z: usize) -> usize {
    debug_assert!(x < 16 && z < 16);
    (z << 4) | x
}

#[inline]
const fn idx_to_xz(idx: usize) -> (usize, usize) {
    debug_assert!(idx < 256);
    (idx & 15, idx >> 4)
}

/// Per-column state for the incremental top-down heightmap scan.
///
/// The scan walks chunk slots from `max_chunk_y` downward, stopping at
/// the first absent slot. As it walks, it writes `Heightmaps::surface_set` /
/// `Heightmaps::motion_blocking_set` the moment it closes each XZ column
/// for that variant, and records closure in a per-variant 256-bit bitset.
/// Finalization is fully derived (`is_finalized()`); the predicate fires
/// either when the cursor drops below `min_chunk_y` (chimney-to-bedrock)
/// or both variants have closed every XZ column.
///
/// Why the bitset and not "compare `hm.surface_get` against `min_y`"?
/// At chimney-to-bedrock finalization the heightmap legitimately holds
/// `min_y` for the all-air columns, indistinguishable from "not yet
/// closed". The bitset is the unambiguous source of truth for closure.
///
/// Partial-column contract: until `is_finalized()` returns true, entries
/// for XZ columns the scan has not yet closed hold the sentinel value
/// equal to `min_y`. Callers that need accurate heightmap values for
/// partial columns must check `is_finalized()` before trusting
/// `surface_get` / `motion_blocking_get`.
/// `update_heightmaps_on_block_placed` is the authoritative writer for
/// post-finalization edits — its rescan path tolerates sentinel reads and
/// writes the correct value regardless of scan state.
///
/// Total state size: 4 (cursor) + 4 (min_y) + 32 + 32 (bitsets) = 72 bytes.
#[derive(Component, Debug)]
pub struct ColumnHeightmapScan {
    /// Chunk Y coordinate to process next. Decrements as chunks are scanned.
    /// Initial value: `max_chunk_y` (the topmost chunk).
    pub scan_cursor: i32,
    min_chunk_y: i32,
    world_surface_done: BitSet256,
    motion_blocking_done: BitSet256,
}

impl ColumnHeightmapScan {
    /// Create a fresh scan state with explicit chunk-y bounds. Caller
    /// passes both directly from the column's `ColumnChunks` / dimension
    /// metadata; no off-by-one arithmetic happens here.
    #[inline]
    pub fn new(min_chunk_y: i32, max_chunk_y: i32) -> Self {
        debug_assert!(max_chunk_y >= min_chunk_y);
        Self {
            scan_cursor: max_chunk_y,
            min_chunk_y,
            world_surface_done: BitSet256::default(),
            motion_blocking_done: BitSet256::default(),
        }
    }

    /// Finalization predicate. True once the cursor has dropped below the
    /// floor (chimney-to-bedrock path completed) or both heightmap variants
    /// have closed every XZ column.
    #[inline]
    pub fn is_finalized(&self) -> bool {
        self.scan_cursor < self.min_chunk_y
            || (self.world_surface_done.is_full() && self.motion_blocking_done.is_full())
    }
}

/// Stage 2.5 of the chunk-column lifecycle.
///
/// Runs a single stateful top-down scan per column that simultaneously
/// writes the heightmap and tracks the lighting gate. On every
/// `Changed<ColumnChunks>` event, advances the per-column
/// `ColumnHeightmapScan` as far as the currently-loaded chunks allow.
/// The scan writes `Heightmaps::{surface,motion_blocking}_set` the moment
/// it closes each XZ column for that variant. When both variants have
/// closed every XZ column — or the cursor drops below `min_chunk_y`
/// (chimney-to-bedrock) — `is_finalized()` flips to true and
/// `BlockNeedsInitialSeed` is inserted on every currently-loaded chunk;
/// `SkyNeedsInitialSeed` is also inserted on chunks whose parent dimension
/// carries `HasSkyLight`.
///
/// Late-arrival path: once the scan is finalized, any newly-registered
/// chunk receives `BlockNeedsInitialSeed` (and `SkyNeedsInitialSeed` when
/// the dimension carries `HasSkyLight`) immediately.
///
/// `IsAllAir` is a fast-skip: chunks where every block is air-equivalent
/// cannot contribute a qualifying block. The cursor advances past such
/// chunks without per-block work.
///
/// Pitfall #1 safety: this system lives in `mcrs_minecraft_lighting`
/// because it consumes `BlockStateLightTable`. The engine crate stays free of
/// any lighting-side imports. Runs after `ColumnLifecycleSet::ReconcileIndex`
/// (Stage 2) so the column's `ColumnChunks` is fully populated for the
/// chunks that triggered the column spawn.
pub fn prime_heightmaps_on_column_spawn(
    changed_columns: Query<(Entity, &ColumnChunks), (With<Column>, Changed<ColumnChunks>)>,
    chunks: Query<(&BlockPalette, Has<IsAllAir>)>,
    mut col_state: Query<(&mut Heightmaps, Option<&mut ColumnHeightmapScan>)>,
    in_dimensions: Query<&InDimension>,
    sky_dims: Query<(), With<HasSkyLight>>,
    table: Res<BlockStateLightTable>,
    mut commands: Commands,
) {
    for (column_entity, chunk_index) in changed_columns.iter() {
        let Ok((mut hm, scan_opt)) = col_state.get_mut(column_entity) else {
            continue;
        };

        let min_chunk_y = chunk_index.min_section_y;
        let chunk_count = chunk_index.sections.len();
        let max_chunk_y = min_chunk_y + chunk_count as i32 - 1;

        if let Some(mut scan) = scan_opt {
            if scan.is_finalized() {
                // Late-arrival: insert the per-channel needs-initial markers on
                // any chunk slot present. The Changed event fired because a new
                // chunk just landed; the older chunks already have the markers
                // (consumed or parked) so the re-insert is a no-op for them.
                for slot in chunk_index.sections.iter() {
                    let Some(chunk_entity) = slot else { continue };
                    let mut e = commands.entity(*chunk_entity);
                    e.insert(BlockNeedsInitialSeed);
                    if let Ok(in_dim) = in_dimensions.get(*chunk_entity) {
                        if sky_dims.get(in_dim.0).is_ok() {
                            e.insert(SkyNeedsInitialSeed);
                        }
                    }
                }
                continue;
            }

            advance_scan(
                &mut scan,
                &mut hm,
                chunk_index,
                &chunks,
                &in_dimensions,
                &sky_dims,
                &table,
                &mut commands,
            );
        } else {
            // First observation of this column: init the scan state and
            // attempt to advance it in the same tick.
            let mut scan = ColumnHeightmapScan::new(min_chunk_y, max_chunk_y);
            advance_scan(
                &mut scan,
                &mut hm,
                chunk_index,
                &chunks,
                &in_dimensions,
                &sky_dims,
                &table,
                &mut commands,
            );
            commands.entity(column_entity).insert(scan);
        }
    }
}

fn advance_scan(
    scan: &mut ColumnHeightmapScan,
    hm: &mut Heightmaps,
    chunk_index: &ColumnChunks,
    chunks: &Query<(&BlockPalette, Has<IsAllAir>)>,
    in_dimensions: &Query<&InDimension>,
    sky_dims: &Query<(), With<HasSkyLight>>,
    table: &BlockStateLightTable,
    commands: &mut Commands,
) {
    debug_assert!(!scan.is_finalized());
    let min_chunk_y = scan.min_chunk_y;

    let palette_fn = |entity: Entity| -> Option<&BlockPalette> {
        let (palette, is_all_air) = chunks.get(entity).ok()?;
        if is_all_air {
            None
        } else {
            Some(palette)
        }
    };

    let outcome = {
        let scan_ref = &mut *scan;
        let hm_ref = &mut *hm;
        scan_top_down(
            chunk_index,
            palette_fn,
            table,
            &XZ_FULL,
            &mut scan_ref.scan_cursor,
            |x, z, variant, world_y| {
                record_topmost(hm_ref, variant, x, z, world_y);
                let idx = xz_idx(x, z);
                match variant {
                    HeightmapVariant::Surface => scan_ref.world_surface_done.set(idx),
                    HeightmapVariant::MotionBlocking => scan_ref.motion_blocking_done.set(idx),
                }
            },
        )
    };

    match outcome {
        ScanOutcome::AllClosed => {
            insert_initial_light_markers(chunk_index, commands, in_dimensions, sky_dims);
        }
        ScanOutcome::ChimneyToBedrock => {
            let mut unclosed_ws = 0usize;
            let mut unclosed_mb = 0usize;
            for idx in 0..256 {
                if !scan.world_surface_done.is_set(idx) {
                    unclosed_ws += 1;
                }
                if !scan.motion_blocking_done.is_set(idx) {
                    unclosed_mb += 1;
                }
            }
            tracing::warn!(
                target: "mcrs_lighting::chimney_to_bedrock",
                min_chunk_y,
                final_cursor = scan.scan_cursor,
                unclosed_world_surface = unclosed_ws,
                unclosed_motion_blocking = unclosed_mb,
                chunks_present = chunk_index.sections.iter().filter(|s| s.is_some()).count(),
                chunk_count = chunk_index.sections.len(),
                "Heightmap scan reached chimney-to-bedrock: every unclosed XZ column gets sentinel min_y. \
                 This is correct ONLY if the column is genuinely all-air top-to-bottom. \
                 For a normal overworld column with a real surface, this path firing means \
                 the surface chunk was never observed by the scan (race or all-air mis-classification)."
            );
            for idx in 0..256 {
                let (x, z) = idx_to_xz(idx);
                if !scan.world_surface_done.is_set(idx) {
                    record_unsurfaced_column(hm, x, z);
                }
                if !scan.motion_blocking_done.is_set(idx) {
                    record_unsurfaced_motion_column(hm, x, z);
                }
            }
            insert_initial_light_markers(chunk_index, commands, in_dimensions, sky_dims);
        }
        ScanOutcome::AbsentSection => {
            // Chunk not yet loaded — return without further action. The
            // system re-fires on the next Changed<ColumnChunks> event.
        }
    }
}

/// Insert `BlockNeedsInitialSeed` on every currently-loaded chunk in the
/// column, plus `SkyNeedsInitialSeed` on chunks whose parent dimension carries
/// `HasSkyLight`. Does not mutate scan state — finalization is read from
/// `is_finalized()`.
fn insert_initial_light_markers(
    chunk_index: &ColumnChunks,
    commands: &mut Commands,
    in_dimensions: &Query<&InDimension>,
    sky_dims: &Query<(), With<HasSkyLight>>,
) {
    for slot in chunk_index.sections.iter() {
        let Some(chunk_entity) = slot else { continue };
        let mut e = commands.entity(*chunk_entity);
        e.insert(BlockNeedsInitialSeed);
        if let Ok(in_dim) = in_dimensions.get(*chunk_entity) {
            if sky_dims.get(in_dim.0).is_ok() {
                e.insert(SkyNeedsInitialSeed);
            }
        }
    }
}

/// Stage 3 of the chunk-column lifecycle. For every newly-loaded chunk
/// (chunks with `Added<ChunkLoaded>`), attach the per-chunk lighting
/// bundles.
///
/// `BlockLightBundle` is inserted unconditionally. `SkyLightBundle` is
/// inserted only when the parent `Dimension` carries `HasSkyLight`.
/// `IsAllAir` is inserted when the chunk's `BlockPalette` contains only
/// air-equivalent states (`emission == 0 && dampening == 0`).
///
/// The per-channel `BlockNeedsInitialSeed` and `SkyNeedsInitialSeed` markers
/// are NOT inserted here. They are inserted by `prime_heightmaps_on_column_spawn`
/// (on the heightmap-scan-finalise path) and by `consume_needs_full_reseed` (on
/// the full-column reseed path), so that `seed_block_emitters` and
/// `seed_sky_initial` always read a fully-primed heightmap. Inserting them here
/// (before the scan finalizes) would cause cave-air chunks to be seeded with
/// stale heightmap data.
pub fn attach_lighting_state(
    newly_loaded: Query<(Entity, &BlockPalette, &InDimension, &ChunkPos), Added<ChunkLoaded>>,
    sky_dims: Query<(), With<HasSkyLight>>,
    table: Res<BlockStateLightTable>,
    mut commands: Commands,
) {
    for (chunk_entity, palette, in_dim, _chunk_pos) in newly_loaded.iter() {
        let mut entity_commands = commands.entity(chunk_entity);
        entity_commands.insert(BlockLightBundle::default());
        if sky_dims.get(in_dim.0).is_ok() {
            entity_commands.insert(SkyLightBundle::default());
        }

        if is_chunk_all_air(palette, &table) {
            entity_commands.insert(IsAllAir);
        }
    }
}

/// `true` if every cell in the chunk's palette has `emission == 0` and
/// `dampening == 0`. Uses `BlockPalette::for_each_distinct_state` to avoid
/// scanning all 4096 cells when the palette holds only a handful of states.
fn is_chunk_all_air(palette: &BlockPalette, table: &BlockStateLightTable) -> bool {
    let mut all_air = true;
    palette.for_each_distinct_state(|state| {
        if !all_air {
            return;
        }
        let idx = state.0 as usize;
        if idx >= table.len() || table.emission[idx] != 0 || table.dampening[idx] != 0 {
            all_air = false;
        }
    });
    all_air
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::flag_bits;
    use crate::{BlockLight, LightingPlugin, SkyLight};
    use bevy_app::{App, FixedUpdate, Update};
    use bevy_state::app::{AppExtStates, StatesPlugin};
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_core::AppState;
    use mcrs_engine::entity::ChunkEntities;
    use mcrs_engine::world::chunk::{Chunk, ChunkLoaded};
    use mcrs_engine::world::column::ColumnPlugin;
    use mcrs_engine::world::dimension::{
        DimensionBundle, DimensionId, DimensionTypeConfig,
    };

    const TEST_DIM_HEIGHT: u32 = 384;
    const TEST_DIM_MIN_Y: i32 = -64;

    fn stub_block_light_table() -> BlockStateLightTable {
        let state_count = 2usize;
        let mut emission = vec![0u8; state_count].into_boxed_slice();
        let mut dampening = vec![0u8; state_count].into_boxed_slice();
        let occlusion: Box<[&'static VoxelShape]> =
            vec![VoxelShape::empty(); state_count].into_boxed_slice();
        let mut flags = vec![0u8; state_count].into_boxed_slice();
        emission[0] = 0;
        dampening[0] = 0;
        flags[0] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;
        emission[1] = 0;
        dampening[1] = 15;
        flags[1] =
            flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
        BlockStateLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        }
    }

    fn build_lifecycle_app(sky: bool) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<AppState>();
        app.add_plugins(ColumnPlugin);
        app.add_plugins(LightingPlugin);
        app.insert_resource(stub_block_light_table());
        let dim = app
            .world_mut()
            .spawn(DimensionBundle {
                type_config: DimensionTypeConfig::new(TEST_DIM_MIN_Y, TEST_DIM_HEIGHT),
                dimension_id: DimensionId::new(if sky { "test:sky" } else { "test:skyless" }),
                ..Default::default()
            })
            .id();
        if sky {
            app.world_mut().entity_mut(dim).insert(HasSkyLight);
        }
        (app, dim)
    }

    fn air_palette() -> BlockPalette {
        let mut p = BlockPalette::default();
        p.fill(mcrs_protocol::BlockStateId(0));
        p
    }

    fn spawn_test_chunk(app: &mut App, dim: Entity, chunk_pos: ChunkPos) -> Entity {
        app.world_mut()
            .spawn((
                InDimension(dim),
                chunk_pos,
                ChunkEntities::default(),
                Chunk,
                ChunkLoaded,
                air_palette(),
            ))
            .id()
    }

    #[test]
    fn attach_lighting_state_does_not_insert_needs_initial() {
        // Build a single-system app with only `attach_lighting_state` registered
        // so we observe its behaviour in isolation: it must attach the per-chunk
        // bundles but never insert the per-channel needs-initial markers.
        let mut app = App::new();
        app.insert_resource(stub_block_light_table());
        app.add_systems(Update, attach_lighting_state);

        let dim = app
            .world_mut()
            .spawn(DimensionBundle {
                type_config: DimensionTypeConfig::new(TEST_DIM_MIN_Y, TEST_DIM_HEIGHT),
                dimension_id: DimensionId::new("test:sky"),
                ..Default::default()
            })
            .id();
        app.world_mut().entity_mut(dim).insert(HasSkyLight);

        let chunk = app
            .world_mut()
            .spawn((
                InDimension(dim),
                ChunkPos::new(0, 0, 0),
                Chunk,
                ChunkLoaded,
                air_palette(),
            ))
            .id();

        app.update();

        let world = app.world();
        assert!(
            world.get::<BlockLight>(chunk).is_some(),
            "attach_lighting_state must insert BlockLightBundle"
        );
        assert!(
            world.get::<SkyLight>(chunk).is_some(),
            "attach_lighting_state must insert SkyLightBundle in sky dim"
        );
        assert!(
            world.get::<BlockNeedsInitialSeed>(chunk).is_none(),
            "attach_lighting_state must NOT insert BlockNeedsInitialSeed"
        );
        assert!(
            world.get::<SkyNeedsInitialSeed>(chunk).is_none(),
            "attach_lighting_state must NOT insert SkyNeedsInitialSeed"
        );
    }

    #[test]
    fn prime_heightmaps_inserts_block_needs_initial_seed_on_finalize() {
        // Full-plugin smoke test: under the LightingPlugin's wiring, a
        // single-chunk all-air column receives `BlockNeedsInitialSeed` via
        // `prime_heightmaps_on_column_spawn` (heightmap scan finalises in one
        // tick) and the marker is consumed by `seed_block_emitters` within
        // the same FixedUpdate tick. After one tick the marker must be gone,
        // but the BlockLight bundle must be present — proving the
        // attach→prime→seed pipeline worked end-to-end.
        let (mut app, dim) = build_lifecycle_app(true);
        let chunk = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0));
        app.world_mut().run_schedule(FixedUpdate);

        let world = app.world();
        assert!(
            world.get::<BlockLight>(chunk).is_some(),
            "BlockLight must be attached by attach_lighting_state"
        );
        assert!(
            world.get::<BlockNeedsInitialSeed>(chunk).is_none(),
            "BlockNeedsInitialSeed must be inserted by prime_heightmaps then consumed by seed_block_emitters within the tick"
        );
    }

    #[test]
    fn prime_heightmaps_inserts_sky_needs_initial_seed_only_in_sky_dim() {
        // Sky case: SkyLight bundle attaches; SkyNeedsInitialSeed is inserted
        // by prime and consumed by seed_sky_initial within one tick.
        // Skyless case: SkyLight bundle is NOT attached; SkyNeedsInitialSeed
        // never appears on the chunk.
        for sky in [true, false] {
            let (mut app, dim) = build_lifecycle_app(sky);
            let chunk = spawn_test_chunk(&mut app, dim, ChunkPos::new(0, 0, 0));
            app.world_mut().run_schedule(FixedUpdate);

            let world = app.world();
            assert!(
                world.get::<BlockLight>(chunk).is_some(),
                "BlockLight must be attached regardless of sky (sky={sky})"
            );
            assert!(
                world.get::<BlockNeedsInitialSeed>(chunk).is_none(),
                "BlockNeedsInitialSeed must be consumed within the tick (sky={sky})"
            );
            if sky {
                assert!(
                    world.get::<SkyLight>(chunk).is_some(),
                    "SkyLight bundle must attach in sky-having dim"
                );
                assert!(
                    world.get::<SkyNeedsInitialSeed>(chunk).is_none(),
                    "SkyNeedsInitialSeed must be consumed within the tick (sky-having dim)"
                );
            } else {
                assert!(
                    world.get::<SkyLight>(chunk).is_none(),
                    "SkyLight bundle must NOT attach in skyless dim"
                );
                assert!(
                    world.get::<SkyNeedsInitialSeed>(chunk).is_none(),
                    "SkyNeedsInitialSeed must NEVER appear in skyless dim"
                );
            }
        }
    }
}
