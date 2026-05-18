// Integration tests for the per-dimension sub-app lifecycle. Each test
// constructs a minimal host `App`, enqueues a synthetic spawn request, drains
// the queue through the production builder, and inspects the resulting
// sub-app population.

use bevy_app::{App, AppLabel, FixedPostUpdate, FixedPreUpdate, FixedUpdate};
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_core::AppState;
use mcrs_engine::world::dimension::{Dimension, DimensionId, DimensionTypeConfig};
use mcrs_engine::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft::world::sub_app_builder::{drain_dim_despawn_queue, drain_dim_spawn_queue};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::block::Block;

mod harness {
    #![allow(dead_code)]
    use super::*;

    pub fn make_stub_block_light_table() -> BlockStateLightTable {
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

    pub fn make_main_app() -> App {
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

    pub fn drive_to_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Playing);
        app.update();
    }

    pub fn enqueue_spawn(app: &mut App, id: &str, sky: bool) {
        app.world_mut()
            .resource_mut::<DimSpawnQueue>()
            .0
            .push(DimSpawnRequest {
                dimension_id: DimensionId::new(id),
                type_config: DimensionTypeConfig::default(),
                has_sky: sky,
            });
    }

}

#[test]
fn dim_subapp_inserted_on_spawn() {
    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(
        app.sub_apps().sub_apps.len(),
        1,
        "exactly one sub-app should be present after one spawn drain"
    );
}

#[test]
fn dim_subapp_removed_on_despawn() {
    use mcrs_minecraft::world::sub_app_builder::DimSubAppHandle;

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 1);

    // The label-anchor entity in the host world is the same value the
    // sub-app was interned under.
    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q
        .iter(app.world())
        .map(|(e, _)| e)
        .collect();
    assert_eq!(handles.len(), 1, "one host-side handle entity per sub-app");
    let label_entity = handles[0];

    // The sub-app's own `World` carries the `Dimension` entity with the bundle.
    let sub_app = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&DimAppLabel(label_entity).intern())
        .expect("sub-app under DimAppLabel");
    let mut q = sub_app.world_mut().query::<(Entity, &Dimension)>();
    let count = q.iter(sub_app.world()).count();
    assert_eq!(count, 1, "exactly one Dimension entity per sub-app world");

    app.world_mut()
        .resource_mut::<DimDespawnQueue>()
        .0
        .push(label_entity);
    drain_dim_despawn_queue(&mut app);
    assert_eq!(
        app.sub_apps().sub_apps.len(),
        0,
        "sub-app should be removed after despawn drain"
    );
}

#[test]
fn dim_worlds_are_isolated() {
    #[derive(Resource)]
    #[allow(dead_code)]
    struct SentinelOverworld(u32);

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    harness::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);
    assert_eq!(app.sub_apps().sub_apps.len(), 2);

    let host_world_dim_count = app
        .world_mut()
        .query::<&Dimension>()
        .iter(app.world())
        .count();
    assert_eq!(
        host_world_dim_count, 0,
        "host world should hold zero Dimension entities"
    );

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    let first = labels[0];
    let second = labels[1];

    app.sub_apps_mut()
        .sub_apps
        .get_mut(&first)
        .expect("first sub-app present")
        .world_mut()
        .insert_resource(SentinelOverworld(42));

    let other_sub_app = app
        .sub_apps()
        .sub_apps
        .get(&second)
        .expect("second sub-app present");
    assert!(
        other_sub_app
            .world()
            .get_resource::<SentinelOverworld>()
            .is_none(),
        "sentinel resource inserted into one sub-app must not appear in the other"
    );
}

#[test]
fn sequential_pump_tick_count() {
    #[derive(Resource, Default)]
    struct TickCounter(u64);

    fn bump(mut counter: ResMut<TickCounter>) {
        counter.0 += 1;
    }

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    harness::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    for label in &labels {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(label)
            .expect("sub-app present");
        sub_app.world_mut().insert_resource(TickCounter::default());
        sub_app.add_systems(FixedUpdate, bump);
    }

    for _ in 0..3 {
        app.update();
    }

    for label in &labels {
        let sub_app = app
            .sub_apps()
            .sub_apps
            .get(label)
            .expect("sub-app present");
        let counter = sub_app.world().resource::<TickCounter>();
        assert_eq!(
            counter.0, 3,
            "each sub-app should have ticked exactly three times"
        );
    }
}

#[test]
fn fixed_pre_and_post_update_advance_once_per_pump() {
    use mcrs_minecraft::world::sub_app_builder::DimSubAppHandle;

    #[derive(Resource, Default)]
    struct PreCounter(u32);

    #[derive(Resource, Default)]
    struct PostCounter(u32);

    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(handles.len(), 1, "one host-side handle entity");
    let label_entity = handles[0];

    {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(&DimAppLabel(label_entity).intern())
            .expect("sub-app under DimAppLabel");
        sub_app.init_resource::<PreCounter>();
        sub_app.init_resource::<PostCounter>();
        sub_app.add_systems(FixedPreUpdate, |mut c: ResMut<PreCounter>| c.0 += 1);
        sub_app.add_systems(FixedPostUpdate, |mut c: ResMut<PostCounter>| c.0 += 1);
    }

    for _ in 0..3 {
        app.update();
    }

    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&DimAppLabel(label_entity).intern())
        .expect("sub-app under DimAppLabel");
    let pre = sub_app.world().resource::<PreCounter>();
    let post = sub_app.world().resource::<PostCounter>();
    assert_eq!(pre.0, 3, "FixedPreUpdate must tick once per host pump");
    assert_eq!(post.0, 3, "FixedPostUpdate must tick once per host pump");
}

#[test]
fn no_per_dim_task_pool() {
    let source: &str = include_str!("../../mcrs_minecraft/src/world/sub_app_builder.rs");
    assert!(
        !source.contains("TaskPoolBuilder"),
        "sub_app_builder.rs must not construct its own task pool"
    );
    assert!(
        !source.contains("ComputeTaskPool::init"),
        "sub_app_builder.rs must rely on the process-global ComputeTaskPool"
    );
}

#[test]
fn registries_present_in_all_subapps() {
    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    harness::enqueue_spawn(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    let host_registry: RegistryAccess = app.world().resource::<RegistryAccess>().clone();
    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 2, "two sub-apps after two spawns");
    for label in &labels {
        let sub_app = app
            .sub_apps()
            .sub_apps
            .get(label)
            .expect("sub-app present");
        let world = sub_app.world();
        let access = world
            .get_resource::<RegistryAccess>()
            .expect("RegistryAccess resource present in sub-app");
        assert!(
            host_registry.shares_inner_with(access),
            "RegistryAccess clone must share the host Arc"
        );
        assert!(
            world.get_resource::<BlockStateLightTable>().is_some(),
            "BlockStateLightTable resource present in sub-app"
        );
        assert!(
            world.get_resource::<StaticRegistry<Block>>().is_some(),
            "StaticRegistry<Block> resource present in sub-app"
        );
    }
}

#[test]
fn time_extracted_into_subapp() {
    let mut app = harness::make_main_app();
    harness::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    app.update();

    let host_fixed = *app.world().resource::<Time<Fixed>>();
    let label = *app
        .sub_apps()
        .sub_apps
        .keys()
        .next()
        .expect("one sub-app present");
    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label)
        .expect("sub-app present");
    let sub_fixed = *sub_app.world().resource::<Time<Fixed>>();
    assert_eq!(
        host_fixed.elapsed(),
        sub_fixed.elapsed(),
        "Time<Fixed>::elapsed should be extracted into the sub-app verbatim"
    );
    assert_eq!(
        host_fixed.delta(),
        sub_fixed.delta(),
        "Time<Fixed>::delta should be extracted into the sub-app verbatim"
    );
}

#[test]
fn eager_spawn_count_matches_dims() {
    use bevy_state::prelude::OnEnter;

    // The production path is `OnEnter(AppState::Playing) →
    // enqueue_dim_spawns_from_preset → DimSpawnQueue → runner drain`. The
    // preset and dimension-type fixtures live behind `pub(crate)` types in
    // `mcrs_minecraft::configuration`, so this test exercises the same
    // OnEnter-then-drain wiring with a small inline system that mirrors what
    // `enqueue_dim_spawns_from_preset` does: push one `DimSpawnRequest` per
    // configured dimension into `DimSpawnQueue`. The drain afterwards is the
    // exact same call the production runner loop makes.
    const EXPECTED_DIMS: &[(&str, bool)] = &[
        ("test:overworld", true),
        ("test:nether", false),
        ("test:end", false),
    ];

    fn enqueue_test_dims(mut spawn_queue: ResMut<DimSpawnQueue>) {
        for (id, has_sky) in EXPECTED_DIMS {
            spawn_queue.0.push(DimSpawnRequest {
                dimension_id: DimensionId::new(*id),
                type_config: DimensionTypeConfig::default(),
                has_sky: *has_sky,
            });
        }
    }

    let mut app = harness::make_main_app();
    app.add_systems(OnEnter(AppState::Playing), enqueue_test_dims);

    harness::drive_to_playing(&mut app);

    assert_eq!(
        app.world().resource::<DimSpawnQueue>().0.len(),
        EXPECTED_DIMS.len(),
        "OnEnter(AppState::Playing) must enqueue one DimSpawnRequest per configured dim"
    );

    drain_dim_spawn_queue(&mut app);

    assert_eq!(
        app.sub_apps().sub_apps.len(),
        EXPECTED_DIMS.len(),
        "drain must materialise one sub-app per enqueued spawn request"
    );

    let host_dim_count = app
        .world_mut()
        .query::<&Dimension>()
        .iter(app.world())
        .count();
    assert_eq!(
        host_dim_count, 0,
        "host world must hold zero Dimension entities — each one lives in its own sub-app"
    );
}
