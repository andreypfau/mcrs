use bevy_app::{FixedUpdate, SubApp};
use bevy_ecs::prelude::Res;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use mcrs_engine_cell::CellIndex;
use mcrs_minecraft_engine::column_store::{ColumnStoreLink, StoredColumnLandSet};
use mcrs_minecraft_engine::stored_column::PendingColumnFacts;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use tracing::warn;

use crate::dimension_plans::DimensionPlans;
use crate::saved_monuments::rebuild_saved_monument_rooms;

pub(crate) fn build_dimension_world(
    host: &World,
    build: &DimensionBuild<'_>,
    sub_app: &mut SubApp,
) {
    let first_cell = sub_app
        .world()
        .get_resource::<OwnedColumns>()
        .is_none_or(|owned| owned.index() == CellIndex(0));
    if first_cell {
        insert_fill_context(host, build, sub_app);
    }
    if let Some(mut store) = sub_app.world_mut().get_resource_mut::<ColumnStoreLink>() {
        store.generate_through_world();
    }
    sub_app.add_systems(
        FixedUpdate,
        rebuild_saved_monument_rooms
            .in_set(TickSet::DomainInput)
            .after(StoredColumnLandSet)
            .run_if(|landed: Option<Res<PendingColumnFacts>>| {
                landed.is_some_and(|landed| !landed.0.is_empty())
            }),
    );
}

fn insert_fill_context(host: &World, build: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    let dimension = build.dimension.location();
    let Some(plan) = host
        .get_resource::<DimensionPlans>()
        .and_then(|plans| plans.0.get(dimension))
    else {
        return;
    };
    let fill = plan.fill_context(build.type_config.min_y, build.type_config.section_count);
    if fill.router.is_some() && fill.program.features.is_none() {
        warn!(%dimension, "no feature program for this dimension; it will place no features");
    }
    sub_app.insert_resource(fill);
}
