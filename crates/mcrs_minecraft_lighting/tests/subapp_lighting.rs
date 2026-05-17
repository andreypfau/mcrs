// Integration test: `LightingPlugin` must compose cleanly into a
// per-dimension sub-app's `World`. The test builds a minimal host `App`,
// drains a synthetic spawn through the production builder, and inspects the
// resulting sub-app's schedule graph for the lighting infrastructure.

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedules;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
use mcrs_engine::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_lighting::converge::LightConvergeSchedule;
use mcrs_minecraft_lighting::sets::LightingSet;
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::block::Block;

fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    let emission = vec![0u8; state_count].into_boxed_slice();
    let dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let flags = vec![0u8; state_count].into_boxed_slice();
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

fn make_main_app() -> App {
    let mut app = App::new();
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(make_stub_block_light_table());
    app.insert_resource(StaticRegistry::<Block>::new());
    app
}

#[test]
fn lighting_plugin_in_subapp() {
    let mut app = make_main_app();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new("test:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 1);

    let label_key = *app
        .sub_apps()
        .sub_apps
        .keys()
        .next()
        .expect("one sub-app present");
    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label_key)
        .expect("sub-app under label");

    let schedules = sub_app.world().resource::<Schedules>();
    assert!(
        schedules.contains(LightConvergeSchedule),
        "LightConvergeSchedule must be registered inside the per-dim sub-app"
    );

    let fixed_update = schedules
        .get(FixedUpdate)
        .expect("FixedUpdate schedule registered in sub-app");
    let graph = fixed_update.graph();
    let emit_dirty_set_key = graph
        .system_sets
        .get_key(LightingSet::EmitDirty.intern())
        .expect("LightingSet::EmitDirty must be a SystemSet in the sub-app's FixedUpdate");
    let hierarchy = graph.hierarchy().graph();
    let emit_dirty_node: bevy_ecs::schedule::NodeId = emit_dirty_set_key.into();
    let has_emit_member = graph
        .systems
        .iter()
        .any(|(key, system, _conds)| {
            let name = format!("{}", system.name());
            (name.contains("emit_block_light_dirty") || name.contains("emit_sky_light_dirty"))
                && hierarchy.contains_edge(emit_dirty_node, key.into())
        });
    assert!(
        has_emit_member,
        "LightingSet::EmitDirty must contain at least one emit_*_light_dirty system in the sub-app"
    );

    // Silence the unused-import lint on DimAppLabel: it is the type used to
    // intern the sub-app key.
    let _ = std::any::type_name::<DimAppLabel>();
}
