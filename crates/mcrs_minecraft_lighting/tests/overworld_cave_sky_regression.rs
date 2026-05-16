// Integration regression: caves below y=0 in a real overworld column must not
// receive sky_light > 0 after the full chunk-generation pipeline converges.
//
// This test drives the actual production pipeline:
//   scheduler → process_completed_columns → ColumnChunks reconciliation
//   → PrimeHeightmaps → AttachLighting → SeedInitialLight
//
// A player observer at (0, 80, 0) triggers the scheduler. All generated columns
// within the view distance are allowed to converge. After convergence, every
// section cell with world_y <= 0 must have sky_light == 0.
//
// The specific cell (-59, -32, -60) (chunk_x=-4, chunk_z=-4, chunk_y=-2, local
// (5, 0, 4)) is checked explicitly as the user-reported reproduction case.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_app::{App, FixedPreUpdate, FixedUpdate};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::state::NextState;

use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::{AppState, StaticRegistry};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::{
    ChunkViewPlugin, PlayerChunkObserver, PlayerViewDistance,
};
use mcrs_engine::world::chunk::{ChunkLoading, ChunkPos};
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionPlugin, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_minecraft::world::chunk::{ColumnScheduler, ChunkPlugin as WorldgenChunkPlugin};
use mcrs_minecraft_lighting::components::{ChunkNeedsInitialLight, LightDirty, SkyLight};
use mcrs_minecraft_lighting::table::BlockLightTable;
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::density_function::proto::{
    DensityFunctionHolder, NoiseParam, ProtoDensityFunction,
};
use mcrs_minecraft_worldgen::bevy::OverworldNoiseRouter;
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_protocol::Ident;
use mcrs_vanilla::block::Block;

const DIM_MIN_Y: i32 = -64;
const DIM_HEIGHT: u32 = 384;

// ---- Asset path resolution --------------------------------------------------

fn workspace_assets_path() -> PathBuf {
    // CARGO_MANIFEST_DIR points to crates/mcrs_minecraft_lighting/ at test time.
    // The workspace assets/ directory is two levels up.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("assets"))
        .expect("unexpected CARGO_MANIFEST_DIR depth")
}

// ---- Noise router from disk (mirrors bench_worldgen.rs) ---------------------

fn walk_json_files(
    base: &std::path::Path,
    dir: &std::path::Path,
    namespace: &str,
    out: &mut Vec<(Ident<String>, Vec<u8>)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_json_files(base, &path, namespace, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            let rel = path.strip_prefix(base).unwrap();
            let name = rel.with_extension("").to_string_lossy().replace('\\', "/");
            let ident_str = format!("{}:{}", namespace, name);
            if let Ok(ident) = Ident::new(ident_str) {
                let data = std::fs::read(&path).unwrap();
                out.push((ident.into(), data));
            }
        }
    }
}

fn resolve_holder(
    id: &Ident<String>,
    holder: &DensityFunctionHolder,
    all: &BTreeMap<Ident<String>, DensityFunctionHolder>,
    out: &mut BTreeMap<Ident<String>, ProtoDensityFunction>,
) {
    if out.contains_key(id) {
        return;
    }
    match holder {
        DensityFunctionHolder::Value(v) => {
            out.insert(id.clone(), ProtoDensityFunction::Constant(v.clone()));
        }
        DensityFunctionHolder::Reference(r) => {
            if let Some(dep) = all.get(r) {
                resolve_holder(id, dep, all, out);
            }
        }
        DensityFunctionHolder::Owned(proto) => {
            out.insert(id.clone(), *proto.clone());
        }
    }
}

fn load_overworld_noise_router(assets_path: &std::path::Path) -> OverworldNoiseRouter {
    let settings_path = assets_path.join("minecraft/worldgen/noise_settings/overworld.json");
    let settings_data = std::fs::read(&settings_path)
        .unwrap_or_else(|e| panic!("failed to read overworld noise settings: {e}"));
    let settings: NoiseGeneratorSettings = serde_json::from_slice(&settings_data)
        .expect("failed to parse overworld noise settings");

    let df_dir = assets_path.join("minecraft/worldgen/density_function");
    let mut df_files = Vec::new();
    walk_json_files(&df_dir, &df_dir, "minecraft", &mut df_files);

    let mut holders: BTreeMap<Ident<String>, DensityFunctionHolder> = BTreeMap::new();
    for (ident, data) in &df_files {
        if let Ok(holder) = serde_json::from_slice::<DensityFunctionHolder>(data) {
            holders.insert(ident.clone(), holder);
        }
    }

    let mut functions: BTreeMap<Ident<String>, ProtoDensityFunction> = BTreeMap::new();
    let holders_snapshot = holders.clone();
    for (ident, holder) in &holders_snapshot {
        resolve_holder(ident, holder, &holders_snapshot, &mut functions);
    }

    let noise_dir = assets_path.join("minecraft/worldgen/noise");
    let mut noise_files = Vec::new();
    walk_json_files(&noise_dir, &noise_dir, "minecraft", &mut noise_files);

    let mut noises: BTreeMap<Ident<String>, NoiseParam> = BTreeMap::new();
    for (ident, data) in &noise_files {
        if let Ok(noise) = serde_json::from_slice::<NoiseParam>(data) {
            noises.insert(ident.clone(), noise);
        }
    }

    // Match the production seed in `NoiseGeneratorSettingsPlugin` (bevy.rs).
    // Test with the same world the live server generates.
    let router = build_functions(&functions, &noises, &settings, 2);
    OverworldNoiseRouter(Arc::new(router))
}

// ---- BlockLightTable from block registry ------------------------------------

fn build_production_block_light_table() -> BlockLightTable {
    use mcrs_minecraft_lighting::table::flag_bits;

    // Register and freeze the block registry in an isolated StaticRegistry
    // so we can build the table without touching the global app state.
    let mut registry: StaticRegistry<Block> = StaticRegistry::new();
    mcrs_vanilla::block::minecraft::register_all_blocks(&mut registry);
    registry.freeze();

    let mut total_states = 0usize;
    for (_id, _loc, block) in registry.iter() {
        let base = block.base_state_id().0 as usize;
        let span = base + block.state_count as usize;
        if span > total_states {
            total_states = span;
        }
    }

    let mut emission = vec![0u8; total_states].into_boxed_slice();
    let mut dampening = vec![0u8; total_states].into_boxed_slice();
    let mut occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); total_states].into_boxed_slice();
    let mut flags = vec![0u8; total_states].into_boxed_slice();

    for (_id, _loc, block) in registry.iter() {
        let base = block.base_state_id().0 as usize;
        for offset in 0..block.state_count {
            let state_id = block
                .base_state_id()
                .0
                .checked_add(offset)
                .expect("state id overflow");
            let state = mcrs_protocol::BlockStateId(state_id);
            let idx = base + offset as usize;
            emission[idx] = block.properties.light_emission.eval(block, state);
            dampening[idx] = block.properties.light_dampening.eval(block, state);
            let occ = block.properties.occlusion.eval(block, state);
            occlusion[idx] = occ;

            let mut f = 0u8;
            if !occ.is_empty() && !occ.occludes_full_block() {
                f |= flag_bits::IS_CONDITIONALLY_OPAQUE;
            }
            if dampening[idx] == 0 {
                f |= flag_bits::PROPAGATES_SKYLIGHT_DOWN;
            }
            if block.properties.can_occlude && dampening[idx] == 15 {
                f |= flag_bits::IS_SOLID_OPAQUE;
            }
            if block.properties.has_collision {
                f |= flag_bits::IS_MOTION_BLOCKING;
            }
            if !block.properties.is_air {
                f |= flag_bits::IS_NOT_AIR;
            }
            flags[idx] = f;
        }
    }

    BlockLightTable { emission, dampening, occlusion, flags }
}

// ---- Convergence helpers -----------------------------------------------------

fn is_scheduler_idle(world: &World) -> bool {
    let sched = world.resource::<ColumnScheduler>();
    sched.pending.is_empty() && sched.in_flight.is_empty()
}

fn has_light_dirty(world: &mut World) -> bool {
    let mut q = world.query_filtered::<(), With<LightDirty>>();
    q.iter(world).next().is_some()
}

fn has_needs_initial_light(world: &mut World) -> bool {
    let mut q = world.query_filtered::<(), With<ChunkNeedsInitialLight>>();
    q.iter(world).next().is_some()
}

fn count_chunk_loading(world: &mut World, dim_entity: Entity) -> usize {
    let mut q = world.query_filtered::<&InDimension, With<ChunkLoading>>();
    q.iter(world).filter(|in_dim| in_dim.0 == dim_entity).count()
}

// ---- Test -------------------------------------------------------------------

#[test]
fn cave_cells_below_y0_have_zero_sky_light_after_real_worldgen() {
    let assets_path = workspace_assets_path();
    if !assets_path.join("minecraft/worldgen/noise_settings/overworld.json").exists() {
        eprintln!(
            "SKIP: assets not found at {}",
            assets_path.display()
        );
        return;
    }

    let router = load_overworld_noise_router(&assets_path);
    let table = build_production_block_light_table();

    // Build the app. AssetPlugin must come first since NoiseGeneratorSettingsPlugin
    // (added by WorldgenChunkPlugin) calls init_asset / register_asset_loader.
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();

    app.add_plugins(AssetPlugin::default());

    // DimensionPlugin adds ColumnPlugin + the engine ticket-based ChunkPlugin.
    app.add_plugins(DimensionPlugin);

    // WorldgenChunkPlugin: priority scheduler + worldgen dispatch systems.
    // Also adds NoiseGeneratorSettingsPlugin (harmless since the router is
    // pre-inserted). dispatch_column_generation is gated on
    // resource_exists::<OverworldNoiseRouter>.
    app.add_plugins(WorldgenChunkPlugin);

    // ChunkViewPlugin drives PlayerChunkObserver → load tickets.
    app.add_plugins(ChunkViewPlugin);

    // LightingPlugin: PrimeHeightmaps → AttachState → Enqueue → Converge.
    app.add_plugins(LightingPlugin);

    // Pre-insert the synchronously-built router and block light table.
    app.insert_resource(router);
    app.insert_resource(table);

    // Spawn the overworld dimension.
    let dim_entity = app
        .world_mut()
        .spawn(DimensionBundle {
            type_config: DimensionTypeConfig::new(DIM_MIN_Y, DIM_HEIGHT),
            dimension_id: DimensionId::new("test:overworld"),
            ..Default::default()
        })
        .id();
    app.world_mut().entity_mut(dim_entity).insert(HasSkyLight);

    // Spawn a player at surface level. PlayerViewDistance default is 12,
    // covering chunk_x/z in [-12, 12], which includes chunk (-4, -4).
    let _player = app
        .world_mut()
        .spawn((
            Player,
            Transform::from_xyz(0.0, 80.0, 0.0),
            InDimension(dim_entity),
            PlayerChunkObserver::default(),
            PlayerViewDistance::default(),
        ))
        .id();

    // Transition to Playing.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);

    // Warm-up ticks for the state machine transition.
    for _ in 0..5 {
        app.world_mut().run_schedule(FixedPreUpdate);
        app.world_mut().run_schedule(FixedUpdate);
    }

    let hard_cap_per_phase = 4000usize;
    let tick_sleep = Duration::from_millis(2);

    // Streaming pattern: the live regression fires for chunks streamed in at
    // the EDGE of view during player movement, not for the initial wave at
    // spawn. Reproduce that by walking the player through several positions
    // so each step triggers a fresh streaming wave whose newly-loaded chunks
    // observe partial column state from prior in-flight work.
    //
    // Path: origin → +X → +X+Z → -X+Z → -X-Z → origin. Each step is 8 chunks
    // (128 blocks) so half the view overlaps with the prior position and half
    // is freshly streamed.
    let path: &[(f64, f64, f64)] = &[
        (0.0, 80.0, 0.0),     // initial spawn (already set above)
        (128.0, 80.0, 0.0),   // east 8 chunks
        (128.0, 80.0, 128.0), // east+south 8 chunks
        (-128.0, 80.0, 128.0),// west of origin
        (-128.0, 80.0, -128.0),
        (0.0, 80.0, 0.0),     // back to origin
    ];

    for (step_idx, &(px, py, pz)) in path.iter().enumerate() {
        if step_idx > 0 {
            // Move the player. update_view fires on Changed<Transform>.
            let mut t = app
                .world_mut()
                .get_mut::<Transform>(_player)
                .expect("player Transform missing");
            t.translation.x = px;
            t.translation.y = py;
            t.translation.z = pz;
        }

        let phase_start = Instant::now();
        let mut converged = false;

        for tick in 0..hard_cap_per_phase {
            app.world_mut().run_schedule(FixedPreUpdate);
            app.world_mut().run_schedule(FixedUpdate);

            let scheduler_idle = is_scheduler_idle(app.world());
            let no_dirty = !has_light_dirty(app.world_mut());
            let no_pending_init = !has_needs_initial_light(app.world_mut());
            let no_loading = count_chunk_loading(app.world_mut(), dim_entity) == 0;

            if scheduler_idle && no_dirty && no_pending_init && no_loading {
                eprintln!(
                    "Phase {} ({:.0}, {:.0}, {:.0}) converged in {} ticks ({:.1}s)",
                    step_idx,
                    px, py, pz,
                    tick + 1,
                    phase_start.elapsed().as_secs_f64()
                );
                converged = true;
                break;
            }

            if tick % 200 == 0 && tick > 0 {
                let loading = count_chunk_loading(app.world_mut(), dim_entity);
                let sched = app.world().resource::<ColumnScheduler>();
                eprintln!(
                    "phase {} tick {}: pending={} in_flight={} loading={} dirty={} needs_init={}",
                    step_idx, tick,
                    sched.pending.len(),
                    sched.in_flight.len(),
                    loading,
                    has_light_dirty(app.world_mut()),
                    has_needs_initial_light(app.world_mut()),
                );
            }

            std::thread::sleep(tick_sleep);
        }

        if !converged {
            let loading = count_chunk_loading(app.world_mut(), dim_entity);
            let sched = app.world().resource::<ColumnScheduler>();
            panic!(
                "phase {} ({:.0}, {:.0}, {:.0}) failed to converge in {} ticks; \
                 scheduler pending={} in_flight={} loading={}",
                step_idx,
                px, py, pz,
                hard_cap_per_phase,
                sched.pending.len(),
                sched.in_flight.len(),
                loading,
            );
        }
    }

    // Collect section entities in the dimension (avoids borrow issues).
    let sections: Vec<(Entity, ChunkPos)> = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &ChunkPos, &InDimension, &SkyLight)>();
        q.iter(app.world())
            .filter(|(_, _, in_dim, _)| in_dim.0 == dim_entity)
            .map(|(e, pos, _, _)| (e, *pos))
            .collect()
    };

    eprintln!("Scanning {} sections for sky_light violations at world_y <= 0", sections.len());

    // Hard check on the user-reported cell. Must be loaded; must be dark.
    // World cell (-59, -32, -60) → chunk_x=-4, chunk_z=-4, chunk_y=-2,
    // local_x = (-59).rem_euclid(16) = 5, local_y = (-32).rem_euclid(16) = 0,
    // local_z = (-60).rem_euclid(16) = 4.
    let reported_chunk = ChunkPos::new(-4, -2, -4);
    let (reported_entity, _) = sections
        .iter()
        .find(|(_, p)| *p == reported_chunk)
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "user-reported chunk {:?} not loaded — final player position should keep it in view",
                reported_chunk
            )
        });
    let sky = app
        .world()
        .get::<SkyLight>(reported_entity)
        .expect("user-reported section missing SkyLight");
    let level = sky.0.get(5, 0, 4);
    assert_eq!(
        level, 0,
        "user-reported cell (-59, -32, -60) has sky_light={level}, expected 0"
    );

    // General scan: every cell at world_y <= 0 must have sky_light == 0.
    let mut first_violation: Option<String> = None;
    let mut checked_cells = 0u64;

    for (section_entity, chunk_pos) in &sections {
        let section_base_world_y = chunk_pos.y * 16;
        let sky = match app.world().get::<SkyLight>(*section_entity) {
            Some(s) => &s.0,
            None => continue,
        };

        for local_y in 0..16usize {
            let world_y = section_base_world_y + local_y as i32;
            if world_y > 0 {
                continue;
            }
            for z in 0..16usize {
                for x in 0..16usize {
                    checked_cells += 1;
                    let level = sky.get(x, local_y, z);
                    if level != 0 && first_violation.is_none() {
                        first_violation = Some(format!(
                            "world ({}, {}, {}) in chunk ({},{},{}) has sky_light={}",
                            chunk_pos.x * 16 + x as i32,
                            world_y,
                            chunk_pos.z * 16 + z as i32,
                            chunk_pos.x, chunk_pos.y, chunk_pos.z,
                            level,
                        ));
                    }
                }
            }
        }
    }

    eprintln!("Checked {checked_cells} cells at world_y <= 0");

    if let Some(msg) = first_violation {
        panic!("Sky light violation below y=0: {msg}");
    }
}
