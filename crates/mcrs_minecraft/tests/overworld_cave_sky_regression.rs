// Integration regression: caves below y=0 in a real overworld column must not
// receive sky_light > 0 after the full chunk-generation pipeline converges.
//
// This test drives the actual production pipeline:
//   scheduler → process_completed_columns → ColumnChunks reconciliation
//   → PrimeHeightmaps → AttachLighting → SeedInitialLight
//
// A player observer at (0, 80, 0) triggers the scheduler. All generated columns
// within the view distance are allowed to converge. After convergence, every
// chunk cell with world_y <= 0 must have sky_light == 0.
//
// The specific cell (-59, -32, -60) (chunk_x=-4, chunk_z=-4, chunk_y=-2, local
// (5, 0, 4)) is checked explicitly as the user-reported reproduction case.

use mcrs_minecraft_block::block::BlockUpdateFlags;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_app::{App, FixedPreUpdate, FixedUpdate};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::state::NextState;

use mcrs_core::AppState;
use mcrs_core::ResourceLocation;
use mcrs_voxel_world::entity::physics::Transform;
use mcrs_voxel_world::entity::player::Player;
use mcrs_voxel_world::entity::player::chunk_view::{
    ChunkViewPlugin, PlayerChunkObserver, PlayerViewDistance,
};
use mcrs_voxel_world::world::dimension::{
    DimensionBundle, DimensionId, DimensionPlugin, DimensionTypeConfig, HasSkyLight, InDimension,
};
use mcrs_voxel_world::world::lifecycle::markers::ChunkLoading;
use mcrs_minecraft::world::chunk::{ChunkPlugin as WorldgenChunkPlugin, ColumnScheduler};
use mcrs_voxel_light::LightingPlugin;
use mcrs_voxel_light::components::{
    BlockBfsPending, BlockNeedsInitialSeed, SkyBfsPending, SkyLight, SkyNeedsInitialSeed,
};
use mcrs_voxel_light::table::BlockStateLightTable;
use mcrs_minecraft_worldgen::bevy::OverworldNoiseRouter;
use mcrs_minecraft_worldgen::density_function::build_functions;
use mcrs_minecraft_worldgen::density_function::proto::{
    DensityFunctionHolder, NoiseParam, ProtoDensityFunction,
};
use mcrs_minecraft_worldgen::proto::NoiseGeneratorSettings;
use mcrs_voxel_math::ChunkPos;

const DIM_MIN_Y: i32 = -64;
const DIM_HEIGHT: u32 = 384;

// ---- Asset path resolution --------------------------------------------------

fn workspace_assets_path() -> PathBuf {
    // The workspace assets/ directory is two levels above any crate manifest.
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
    out: &mut Vec<(ResourceLocation, Vec<u8>)>,
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
            if let Ok(ident) = ResourceLocation::parse(&ident_str) {
                let data = std::fs::read(&path).unwrap();
                out.push((ident, data));
            }
        }
    }
}

fn resolve_holder(
    id: &ResourceLocation,
    holder: &DensityFunctionHolder,
    all: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
    out: &mut BTreeMap<ResourceLocation, ProtoDensityFunction>,
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
    let settings: NoiseGeneratorSettings =
        serde_json::from_slice(&settings_data).expect("failed to parse overworld noise settings");

    let df_dir = assets_path.join("minecraft/worldgen/density_function");
    let mut df_files = Vec::new();
    walk_json_files(&df_dir, &df_dir, "minecraft", &mut df_files);

    let mut holders: BTreeMap<ResourceLocation, DensityFunctionHolder> = BTreeMap::new();
    for (ident, data) in &df_files {
        if let Ok(holder) = serde_json::from_slice::<DensityFunctionHolder>(data) {
            holders.insert(ident.clone(), holder);
        }
    }

    let mut functions: BTreeMap<ResourceLocation, ProtoDensityFunction> = BTreeMap::new();
    let holders_snapshot = holders.clone();
    for (ident, holder) in &holders_snapshot {
        resolve_holder(ident, holder, &holders_snapshot, &mut functions);
    }

    let noise_dir = assets_path.join("minecraft/worldgen/noise");
    let mut noise_files = Vec::new();
    walk_json_files(&noise_dir, &noise_dir, "minecraft", &mut noise_files);

    let mut noises: BTreeMap<ResourceLocation, NoiseParam> = BTreeMap::new();
    for (ident, data) in &noise_files {
        if let Ok(noise) = serde_json::from_slice::<NoiseParam>(data) {
            noises.insert(ident.clone(), noise);
        }
    }

    // Match the production seed in `NoiseGeneratorSettingsPlugin` (bevy.rs).
    // Test with the same world the live server generates.
    let router = build_functions(
        &functions,
        &noises,
        &settings,
        2,
        mcrs_voxel_storage::VoxelId(1),
        mcrs_voxel_storage::VoxelId(86),
    );
    OverworldNoiseRouter(Arc::new(router))
}

// ---- BlockStateLightTable from block registry ------------------------------------

fn build_production_block_light_table() -> (
    BlockStateLightTable,
    mcrs_vanilla::block::definition::Blocks,
) {
    let mut app = App::new();
    app.add_plugins(bevy_app::TaskPoolPlugin::default());
    app.add_plugins(bevy_asset::AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    let asset_server = app.world().resource::<bevy_asset::AssetServer>().clone();
    let (definitions, _) = mcrs_vanilla::block::definition::load_block_definitions(&asset_server)
        .expect("the block definition corpus loads");
    app.insert_resource(mcrs_vanilla::block::definition::Blocks(Arc::new(
        definitions,
    )));
    app.add_systems(
        bevy_app::Startup,
        mcrs_minecraft::block_light_table::build_block_light_table,
    );
    app.update();
    (
        app.world().resource::<BlockStateLightTable>().clone(),
        app.world()
            .resource::<mcrs_vanilla::block::definition::Blocks>()
            .clone(),
    )
}

// ---- Convergence helpers -----------------------------------------------------

fn is_scheduler_idle(world: &World) -> bool {
    let sched = world.resource::<ColumnScheduler>();
    sched.pending.is_empty() && sched.in_flight.is_empty()
}

fn has_light_dirty(world: &mut World) -> bool {
    let mut q = world.query_filtered::<(), Or<(With<BlockBfsPending>, With<SkyBfsPending>)>>();
    q.iter(world).next().is_some()
}

fn has_needs_initial_light(world: &mut World) -> bool {
    let mut q =
        world.query_filtered::<(), Or<(With<BlockNeedsInitialSeed>, With<SkyNeedsInitialSeed>)>>();
    q.iter(world).next().is_some()
}

fn count_chunk_loading(world: &mut World, dim_entity: Entity) -> usize {
    let mut q = world.query_filtered::<&InDimension, With<ChunkLoading>>();
    q.iter(world)
        .filter(|in_dim| in_dim.0 == dim_entity)
        .count()
}

// ---- Test -------------------------------------------------------------------

#[test]
fn cave_cells_below_y0_have_zero_sky_light_after_real_worldgen() {
    let assets_path = workspace_assets_path();
    if !assets_path
        .join("minecraft/worldgen/noise_settings/overworld.json")
        .exists()
    {
        eprintln!("SKIP: assets not found at {}", assets_path.display());
        return;
    }

    let router = load_overworld_noise_router(&assets_path);
    let (table, blocks) = build_production_block_light_table();

    // Build the app. AssetPlugin must come first since NoiseGeneratorSettingsPlugin
    // (added by WorldgenChunkPlugin) calls init_asset / register_asset_loader.
    let mut app = App::new();
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();

    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });

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
    app.add_plugins(LightingPlugin::<BlockUpdateFlags>::default());

    // Pre-insert the synchronously-built router and block light table.
    app.insert_resource(router);
    app.insert_resource(table);
    app.insert_resource(blocks);

    // Spawn the overworld dimension.
    let dim_entity = app
        .world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new("test:overworld"),
            DimensionTypeConfig::new(DIM_MIN_Y, DIM_HEIGHT),
        ))
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
        (0.0, 80.0, 0.0),      // initial spawn (already set above)
        (128.0, 80.0, 0.0),    // east 8 chunks
        (128.0, 80.0, 128.0),  // east+south 8 chunks
        (-128.0, 80.0, 128.0), // west of origin
        (-128.0, 80.0, -128.0),
        (0.0, 80.0, 0.0), // back to origin
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
                    px,
                    py,
                    pz,
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
                    "phase {} tick {}: parked={} in_flight={} loading={} dirty={} needs_init={}",
                    step_idx,
                    tick,
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
                 scheduler parked={} in_flight={} loading={}",
                step_idx,
                px,
                py,
                pz,
                hard_cap_per_phase,
                sched.pending.len(),
                sched.in_flight.len(),
                loading,
            );
        }
    }

    // Collect chunk entities in the dimension (avoids borrow issues).
    let chunks: Vec<(Entity, ChunkPos)> = {
        let mut q = app
            .world_mut()
            .query::<(Entity, &ChunkPos, &InDimension, &SkyLight)>();
        q.iter(app.world())
            .filter(|(_, _, in_dim, _)| in_dim.0 == dim_entity)
            .map(|(e, pos, _, _)| (e, *pos))
            .collect()
    };

    eprintln!(
        "Scanning {} chunks for sky_light violations at world_y <= 0",
        chunks.len()
    );

    // Hard check on the user-reported cell. Must be loaded; must be dark.
    // World cell (-59, -32, -60) → chunk_x=-4, chunk_z=-4, chunk_y=-2,
    // local_x = (-59).rem_euclid(16) = 5, local_y = (-32).rem_euclid(16) = 0,
    // local_z = (-60).rem_euclid(16) = 4.
    let reported_chunk = ChunkPos::new(-4, -2, -4);
    let (reported_entity, _) = chunks
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
        .expect("user-reported chunk missing SkyLight");
    let level = sky.0.get(5, 0, 4);
    assert_eq!(
        level, 0,
        "user-reported cell (-59, -32, -60) has sky_light={level}, expected 0"
    );

    // General scan: every cell at world_y <= 0 must have sky_light == 0.
    let mut first_violation: Option<String> = None;
    let mut checked_cells = 0u64;

    for (chunk_entity, chunk_pos) in &chunks {
        let chunk_base_world_y = chunk_pos.y * 16;
        let sky = match app.world().get::<SkyLight>(*chunk_entity) {
            Some(s) => &s.0,
            None => continue,
        };

        for local_y in 0..16usize {
            let world_y = chunk_base_world_y + local_y as i32;
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
                            chunk_pos.x,
                            chunk_pos.y,
                            chunk_pos.z,
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
