use bevy_app::{App, FixedPostUpdate, FixedUpdate, SubApp};
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, ScheduleLabel};
use bevy_time::{Fixed, Real, Time, Virtual};
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_engine::world::dimension::{
    DimensionBundle, DimensionId, DimensionPlugin, DimensionTypeConfig, HasSkyLight,
};
use mcrs_engine::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_minecraft_lighting::LightingPlugin;
use mcrs_vanilla::block::Block;

/// Read-only data handed to each per-dimension sub-app at construction.
///
/// `registry_access` is an `Arc<Inner>` newtype, so the clone here is an
/// atomic refcount bump. `block_light_table` is a `Box<[…]>`-backed slab that
/// is computed once at world-freeze and copied per dim; the payload is bounded
/// by `total_block_state_count`, which is in the low thousands. The static
/// block registry stores `&'static Block` values; cloning it duplicates the
/// `Vec`/`HashMap` spines but no block data.
#[derive(Clone)]
pub struct DimRegistryBundle {
    pub registry_access: RegistryAccess,
    pub block_light_table: BlockStateLightTable,
    pub static_block_registry: StaticRegistry<Block>,
}

/// Collect the registry resources that every dim sub-app needs from the host
/// world. Called once per drain cycle so a fresh snapshot of the host-side
/// resources is available to every spawn request in that cycle.
pub fn gather_dim_registries(world: &bevy_ecs::world::World) -> DimRegistryBundle {
    DimRegistryBundle {
        registry_access: world.resource::<RegistryAccess>().clone(),
        block_light_table: world.resource::<BlockStateLightTable>().clone(),
        static_block_registry: world.resource::<StaticRegistry<Block>>().clone(),
    }
}

/// Materialise a per-dimension sub-app and return the `DimAppLabel` key as
/// an `Entity`.
///
/// Constraints encoded here:
/// - `update_schedule = Some(FixedUpdate.intern())` so the sub-app runs
///   `FixedUpdate` exactly once per host pump (not `FixedMain`, whose
///   accumulator would consume host-extracted `Time<Fixed>` into 0 or 2
///   ticks).
/// - The label `Entity` is allocated from the host world's `Entities`
///   allocator so labels are globally unique across all sub-apps. Each
///   sub-app `World` would otherwise allocate the same low-index `Entity`
///   value, which would collide in the `DimAppLabel(Entity)` interned key.
///   The host world does not hold a `Dimension`-tagged entity — the label
///   entity is reserved (no `Dimension` component) and exists only to
///   anchor the label.
/// - A separate `Dimension` entity lives inside the sub-app's `World`,
///   carrying the per-dim components that the simulation queries against.
pub fn spawn_dim_subapp(
    app: &mut App,
    request: &DimSpawnRequest,
    registries: &DimRegistryBundle,
) -> Entity {
    let label_entity = app.world_mut().spawn(DimSubAppHandle).id();

    let mut sub_app = SubApp::new();

    sub_app.update_schedule = Some(FixedUpdate.intern());
    sub_app.add_schedule(Schedule::new(FixedUpdate));
    sub_app.add_schedule(Schedule::new(FixedPostUpdate));

    sub_app.add_plugins(DimensionPlugin);
    sub_app.add_plugins(LightingPlugin);

    sub_app.insert_resource(registries.registry_access.clone());
    sub_app.insert_resource(registries.block_light_table.clone());
    sub_app.insert_resource(registries.static_block_registry.clone());

    // Seed the time resources so any `Res<Time<…>>` read during the first
    // pump (before `set_extract` runs) gets a valid default. The extract
    // closure below overwrites these every tick.
    sub_app.insert_resource(Time::<Fixed>::default());
    sub_app.insert_resource(Time::<Virtual>::default());
    sub_app.insert_resource(Time::<Real>::default());
    sub_app.insert_resource(Time::<()>::default());

    sub_app.set_extract(|main_world, sub_world| {
        if let Some(time_fixed) = main_world.get_resource::<Time<Fixed>>() {
            sub_world.insert_resource(*time_fixed);
        }
        if let Some(time_virtual) = main_world.get_resource::<Time<Virtual>>() {
            sub_world.insert_resource(*time_virtual);
        }
        if let Some(time_real) = main_world.get_resource::<Time<Real>>() {
            sub_world.insert_resource(*time_real);
        }
        if let Some(time) = main_world.get_resource::<Time<()>>() {
            sub_world.insert_resource(*time);
        }
    });

    let dim_entity = sub_app
        .world_mut()
        .spawn(DimensionBundle {
            dimension_id: request.dimension_id.clone(),
            type_config: request.type_config,
            ..Default::default()
        })
        .id();
    if request.has_sky {
        sub_app
            .world_mut()
            .entity_mut(dim_entity)
            .insert(HasSkyLight);
    }

    sub_app.finish();
    sub_app.cleanup();

    app.insert_sub_app(DimAppLabel(label_entity), sub_app);
    label_entity
}

/// Marker component placed on the host-world entity that anchors a
/// `DimAppLabel`. The entity carries no other state — it exists purely to
/// allocate a `World`-unique ID for use as the sub-app label key.
#[derive(bevy_ecs::component::Component)]
pub struct DimSubAppHandle;

/// Drain the `DimSpawnQueue` resource on the host world and materialise a
/// sub-app for each request. Called from outside the ECS run loop because
/// `App::insert_sub_app` requires `&mut App`.
pub fn drain_dim_spawn_queue(app: &mut App) {
    let requests: Vec<DimSpawnRequest> = std::mem::take(
        &mut app.world_mut().resource_mut::<DimSpawnQueue>().0,
    );
    if requests.is_empty() {
        return;
    }
    let bundle = gather_dim_registries(app.world());
    for request in requests {
        spawn_dim_subapp(app, &request, &bundle);
    }
}

/// Drain the `DimDespawnQueue` resource on the host world and tear down the
/// matching sub-apps. Called from outside the ECS run loop because
/// `App::remove_sub_app` requires `&mut App`.
pub fn drain_dim_despawn_queue(app: &mut App) {
    let entities: Vec<Entity> = std::mem::take(
        &mut app.world_mut().resource_mut::<DimDespawnQueue>().0,
    );
    for entity in entities {
        app.remove_sub_app(DimAppLabel(entity));
        // Free the host-side label-anchor entity so the host world's
        // dimension-handle archetype matches the live sub-app population.
        if let Ok(mut entity_mut) = app.world_mut().get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
}
