// Heightmap Y-storage convention: each entry stores `Y + 1` of the topmost
// cell satisfying the predicate (the empty cell on top of the topmost solid /
// motion-blocking cell). An air-only column reads back `min_y` because the
// PackedBitStorage backing is zero-initialized and `surface_get` adds `min_y`
// to the unsigned stored value.
//
// This convention MUST stay identical to `prime_heightmaps_on_column_spawn`
// in `lifecycle.rs`; the two systems write to the same column-level state and
// any drift between them would silently corrupt heightmaps after the first
// place or break.
//
// Early-out predicate: distinguishes places from breaks via `old_state` vs
// `new_state` flags. A place can only raise the stored value if the new state
// satisfies the predicate AND lies strictly above the current surface. A break
// can only lower the stored value if the old state satisfied the predicate AND
// the broken cell is the one currently recorded as the topmost satisfying cell
// (i.e. `placed_y + 1 == current_surface`). When neither condition holds for
// both heightmaps, the rescan is skipped. The earlier `y + 2 <= current_height`
// form ignored old/new states and mishandled breaks of the topmost cell.
//
// Concurrency: `Query<&mut Heightmaps>` plus a separate `Query<&BlockPalette>`
// give the scheduler exclusive write access to heightmap state for the
// duration of the system; no manual locking is needed.
use crate::heightmap::{scan_top_down, HeightmapVariant};
use crate::table::{flag_bits, BlockLightTable};
use bevy_ecs::entity::EntityHashMap;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Entity, Local, Query, Res};
use mcrs_engine::world::column::{Heightmaps, InColumn, ColumnChunks};
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_minecraft_block::palette::BlockPalette;

/// HEIGHT-02 eager fused two-type heightmap update. Reads
/// `MessageReader<BlockPlaced>` and updates `Heightmaps` on the affected
/// `Column`. Applies the `y + 2 <= current_height` early-out per type;
/// falls back to a single top-down rescan when the early-out fails.
///
/// Runs in `FixedUpdate` with `.after(apply_set_block_request)` so the
/// `MessageReader<BlockPlaced>` sees this tick's writes; the
/// `FixedUpdate -> FixedPostUpdate` schedule boundary provides ordering
/// against `update_client_blocks` so downstream codec reads in
/// `FixedPostUpdate` observe up-to-date heightmap state.
///
/// PAR-02: Events are partitioned by column entity into a reused
/// `Local<EntityHashMap<...>>`, then the per-column rescan body runs via
/// `par_iter_mut` over the column query. Each parallel task owns the entire
/// bucket of events for its column, so two tasks never touch the same
/// `Heightmaps` storage; the column-level scheduler grant from
/// `Query<&mut Heightmaps>` plus per-entity-disjoint task assignment makes
/// this safe without manual locking.
pub fn update_heightmaps_on_block_placed(
    mut reader: MessageReader<BlockPlaced>,
    chunks: Query<&InColumn>,
    mut columns: Query<(Entity, &mut Heightmaps, &ColumnChunks)>,
    palettes: Query<&BlockPalette>,
    table: Res<BlockLightTable>,
    mut partitions: Local<EntityHashMap<Vec<BlockPlaced>>>,
) {
    // Reuse the EntityHashMap across ticks but clear each bucket so columns
    // that receive BlockPlaced events tick-after-tick amortise the
    // hash-insert cost.
    for bucket in partitions.values_mut() {
        bucket.clear();
    }

    for placed in reader.read() {
        let Ok(in_column) = chunks.get(placed.chunk) else {
            continue;
        };
        partitions.entry(in_column.0).or_default().push(*placed);
    }

    // One task per column entity; the per-entity body owns the entire bucket
    // of events for that column so two parallel tasks never touch the same
    // Heightmaps storage. Columns absent from the partition map fast-return.
    let partitions_ref = &*partitions;
    let palettes_ref = &palettes;
    let table_ref = &*table;
    columns
        .par_iter_mut()
        .for_each(|(col_entity, mut heightmaps, chunk_index)| {
            let Some(events) = partitions_ref.get(&col_entity) else {
                return;
            };
            if events.is_empty() {
                return;
            }

            let min_y = heightmaps.min_y();
            let max_y = min_y + heightmaps.height() as i32 - 1;

            for placed in events {
                let x = (placed.block_pos.x & 15) as usize;
                let z = (placed.block_pos.z & 15) as usize;
                let placed_y = placed.block_pos.y;

                if placed_y < min_y || placed_y > max_y {
                    // Skip silently here; the post-pass below emits the
                    // warning serially so tracing output stays coherent
                    // across columns.
                    continue;
                }

                let current_surface = heightmaps.surface_get(x, z);
                let current_motion = heightmaps.motion_blocking_get(x, z);

                let old_flags = table_ref.flags_for(placed.old_state);
                let new_flags = table_ref.flags_for(placed.new_state);
                let old_was_surface = (old_flags & flag_bits::IS_NOT_AIR) != 0;
                let new_is_surface = (new_flags & flag_bits::IS_NOT_AIR) != 0;
                let old_was_motion = (old_flags & flag_bits::IS_MOTION_BLOCKING) != 0;
                let new_is_motion = (new_flags & flag_bits::IS_MOTION_BLOCKING) != 0;

                let placed_y_plus_one = placed_y + 1;
                let surface_could_raise = new_is_surface && placed_y_plus_one > current_surface;
                let surface_could_lower = old_was_surface && placed_y_plus_one >= current_surface;
                let motion_could_raise = new_is_motion && placed_y_plus_one > current_motion;
                let motion_could_lower = old_was_motion && placed_y_plus_one >= current_motion;

                if !surface_could_raise
                    && !surface_could_lower
                    && !motion_could_raise
                    && !motion_could_lower
                {
                    continue;
                }

                let (new_surface, new_motion) =
                    rescan_column_xz(chunk_index, palettes_ref, table_ref, x, z, min_y);
                heightmaps.surface_set(x, z, new_surface);
                heightmaps.motion_blocking_set(x, z, new_motion);
            }
        });

    // Serial follow-up: emit the out-of-dimension-Y warning for any events
    // that fell outside their column's [min_y, max_y]. The par_iter_mut body
    // skips these silently because tracing inside a parallel task would
    // interleave warning lines across columns.
    for (col_entity, events) in partitions.iter() {
        if events.is_empty() {
            continue;
        }
        let Ok((_, heightmaps, _)) = columns.get(*col_entity) else {
            continue;
        };
        let min_y = heightmaps.min_y();
        let max_y = min_y + heightmaps.height() as i32 - 1;
        for placed in events {
            let placed_y = placed.block_pos.y;
            if placed_y < min_y || placed_y > max_y {
                tracing::warn!(
                    block_pos = ?placed.block_pos,
                    min_y,
                    max_y,
                    "BlockPlaced outside dimension Y; ignored by heightmap"
                );
            }
        }
    }
}

fn rescan_column_xz(
    chunk_index: &ColumnChunks,
    palettes: &Query<&BlockPalette>,
    table: &BlockLightTable,
    x: usize,
    z: usize,
    min_y: i32,
) -> (i32, i32) {
    let mut world_surface_raw: Option<i32> = None;
    let mut motion_blocking_raw: Option<i32> = None;

    let palette_fn = |entity: Entity| -> Option<&BlockPalette> { palettes.get(entity).ok() };

    let xz = [(x, z)];
    let mut cursor = chunk_index.min_section_y + chunk_index.sections.len() as i32 - 1;

    let _ = scan_top_down(
        chunk_index,
        palette_fn,
        table,
        &xz,
        &mut cursor,
        |_x, _z, variant, world_y| match variant {
            HeightmapVariant::Surface => world_surface_raw = Some(world_y),
            HeightmapVariant::MotionBlocking => motion_blocking_raw = Some(world_y),
        },
    );

    (
        world_surface_raw.map(|y| y + 1).unwrap_or(min_y),
        motion_blocking_raw.map(|y| y + 1).unwrap_or(min_y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::flag_bits;
    use bevy_app::{App, Update};
    use bevy_ecs::message::Messages;
    use mcrs_core::voxel_shape::VoxelShape;
    use mcrs_engine::world::block::BlockPos;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_engine::world::column::{Column, ColumnChunks, Heightmaps, InColumn};
    use mcrs_minecraft_block::block::BlockUpdateFlags;
    use mcrs_minecraft_block::palette::BlockPalette;
    use mcrs_protocol::BlockStateId;

    const AIR: BlockStateId = BlockStateId(0);
    const SOLID: BlockStateId = BlockStateId(1);

    // Dimension shape: one section (chunk_y = 0), min_y = 0, height = 16. Keeps
    // the fixture small while still exercising the full rescan path: solid
    // floor at y in 0..=3, then air, then the test event places SOLID above.
    const DIM_MIN_Y: i32 = 0;
    const DIM_HEIGHT: u32 = 16;

    fn make_test_table() -> BlockLightTable {
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
        flags[1] = flag_bits::IS_NOT_AIR
            | flag_bits::IS_SOLID_OPAQUE
            | flag_bits::IS_MOTION_BLOCKING;
        BlockLightTable {
            emission,
            dampening,
            occlusion,
            flags,
        }
    }

    fn build_app() -> App {
        let mut app = App::new();
        app.add_message::<BlockPlaced>();
        app.insert_resource(make_test_table());
        app.add_systems(Update, update_heightmaps_on_block_placed);
        app
    }

    // Spawn a column with one real chunk at chunk_y=0; palette starts with a
    // solid floor in y=0..=3 so the post-prime baseline surface is 4 for
    // every (x,z).
    fn spawn_column_with_chunk(app: &mut App) -> (Entity, Entity) {
        let column = app
            .world_mut()
            .spawn((
                Column,
                Heightmaps::with_min_y(DIM_HEIGHT, DIM_MIN_Y),
                ColumnChunks::new(0, 1),
            ))
            .id();

        let mut palette = BlockPalette::default();
        palette.fill(AIR);
        for y in 0..=3i32 {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    palette.set(BlockPos::new(x, y, z), SOLID);
                }
            }
        }
        let chunk = app
            .world_mut()
            .spawn((ChunkPos::new(0, 0, 0), InColumn(column), palette))
            .id();

        app.world_mut()
            .get_mut::<ColumnChunks>(column)
            .unwrap()
            .set_loaded(0, chunk);

        // Prime the heightmap so the rescan early-out actually triggers (the
        // production prime stage normally seeds this; the system under test
        // here is only the eager update).
        let mut h = app.world_mut().get_mut::<Heightmaps>(column).unwrap();
        for z in 0..16usize {
            for x in 0..16usize {
                h.surface_set(x, z, 4);
                h.motion_blocking_set(x, z, 4);
            }
        }
        (column, chunk)
    }

    fn write_placed(app: &mut App, placed: BlockPlaced) {
        app.world_mut()
            .resource_mut::<Messages<BlockPlaced>>()
            .write(placed);
    }

    fn snapshot_heightmaps(app: &App, columns: &[Entity]) -> Vec<(usize, Vec<i32>, Vec<i32>)> {
        let mut out = Vec::with_capacity(columns.len());
        for (i, c) in columns.iter().enumerate() {
            let h = app.world().get::<Heightmaps>(*c).unwrap();
            let mut surface = Vec::with_capacity(256);
            let mut motion = Vec::with_capacity(256);
            for z in 0..16usize {
                for x in 0..16usize {
                    surface.push(h.surface_get(x, z));
                    motion.push(h.motion_blocking_get(x, z));
                }
            }
            out.push((i, surface, motion));
        }
        out
    }

    // splitmix64 + Fisher-Yates; matches the deterministic shuffle used by the
    // enqueue-side determinism tests so the seed space is comparable.
    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn deterministic_shuffle<T>(slice: &mut [T], seed: u64) {
        let mut state = seed;
        let n = slice.len();
        for i in (1..n).rev() {
            let r = (splitmix64(&mut state) % ((i + 1) as u64)) as usize;
            slice.swap(i, r);
        }
    }

    #[test]
    fn heightmap_update_dirty_set_is_message_order_independent() {
        // 32 events across 4 distinct columns (8 events per column), each
        // event targeting a unique (x,z) inside its column. Because
        // `update_heightmaps_on_block_placed` rescans the palette from scratch
        // and each event writes to a distinct (x,z), the per-column bucket is
        // commutative — the same events in any order must produce the same
        // post-pass Heightmaps state.
        const N_COLUMNS: usize = 4;
        const EVENTS_PER_COLUMN: usize = 8;

        // Use proto chunk entities + a stable proto->real mapping per run so
        // event identity stays anchored across shuffles. Collecting unique
        // chunks from the shuffled stream would re-index per shuffle and
        // defeat the test.
        let mut proto_app = App::new();
        let proto_chunks: Vec<Entity> = (0..N_COLUMNS)
            .map(|_| proto_app.world_mut().spawn_empty().id())
            .collect();

        // Build the event prototype list. (xi, zi) is unique within each
        // column so the column's bucket is order-independent regardless of
        // how the test event stream is shuffled.
        struct Placement {
            col_index: usize,
            x: i32,
            y: i32,
            z: i32,
        }
        let mut placements: Vec<Placement> = Vec::with_capacity(N_COLUMNS * EVENTS_PER_COLUMN);
        for ci in 0..N_COLUMNS {
            for ei in 0..EVENTS_PER_COLUMN {
                // Distinct (x,z) per event within the column: walk a
                // 4x2 grid offset by the column index so cross-column events
                // don't collide either.
                let x = (ei % 4) as i32;
                let z = ((ei / 4) + ci) as i32 % 16;
                let y = 10i32; // strictly above the solid-floor surface=4
                placements.push(Placement {
                    col_index: ci,
                    x,
                    y,
                    z,
                });
            }
        }

        // Prototype event list: the `.chunk` field holds the proto chunk
        // entity; per-run code below rewrites it to the real chunk entity via
        // a stable proto -> real mapping.
        let baseline_events: Vec<BlockPlaced> = placements
            .iter()
            .map(|p| BlockPlaced {
                chunk: proto_chunks[p.col_index],
                chunk_pos: ChunkPos::new(0, 0, 0),
                block_pos: BlockPos::new(p.x, p.y, p.z),
                old_state: AIR,
                new_state: SOLID,
                flags: BlockUpdateFlags::all(),
            })
            .collect();

        let run_with_events = |events: &[BlockPlaced]| -> Vec<(usize, Vec<i32>, Vec<i32>)> {
            let mut app = build_app();
            let mut real_columns: Vec<Entity> = Vec::with_capacity(N_COLUMNS);
            let mut real_chunks: Vec<Entity> = Vec::with_capacity(N_COLUMNS);
            for _ in 0..N_COLUMNS {
                let (col, chunk) = spawn_column_with_chunk(&mut app);
                real_columns.push(col);
                real_chunks.push(chunk);
            }
            let proto_to_real: std::collections::HashMap<Entity, Entity> = proto_chunks
                .iter()
                .enumerate()
                .map(|(i, p)| (*p, real_chunks[i]))
                .collect();

            // Mutate the palette per event before the system runs, mirroring
            // what `apply_set_block_request` does in production. The rescan
            // reads the palette directly, so the post-pass state depends only
            // on the final palette state — not on event order — provided no
            // two events touch the same (x,y,z).
            for placed in events {
                let mut remapped = *placed;
                remapped.chunk = *proto_to_real.get(&placed.chunk).unwrap();
                app.world_mut()
                    .get_mut::<BlockPalette>(remapped.chunk)
                    .unwrap()
                    .set(remapped.block_pos, remapped.new_state);
                write_placed(&mut app, remapped);
            }
            app.update();
            snapshot_heightmaps(&app, &real_columns)
        };

        let baseline = run_with_events(&baseline_events);

        // Sanity: the baseline must actually mutate something. If every event
        // skipped due to early-out, the test would assert nothing.
        let any_raised = baseline
            .iter()
            .any(|(_, surface, _)| surface.iter().any(|&y| y > 4));
        assert!(
            any_raised,
            "baseline must raise at least one surface cell; otherwise the test exercises nothing"
        );

        // ≥ 4 shuffles required by the plan; use 6 for extra coverage matching
        // the enqueue-side determinism tests.
        for seed in 1u64..=6 {
            let mut shuffled = baseline_events.clone();
            deterministic_shuffle(&mut shuffled, seed);
            let actual = run_with_events(&shuffled);
            assert_eq!(
                actual, baseline,
                "per-column heightmap state must match baseline under seed {seed}"
            );
        }
    }
}
