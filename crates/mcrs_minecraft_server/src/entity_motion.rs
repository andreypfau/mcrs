use bevy_app::FixedUpdate;
use bevy_app::SubApp;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::block_simulation::BlockSimulationSet;
use mcrs_minecraft_engine_brain::MobBrainSet;
use mcrs_minecraft_engine_damage::DamageSet;
use mcrs_minecraft_engine_entity_motion::MotionSteps;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_physics::EntityMotionPlugin;
use mcrs_minecraft_engine_physics::EntityPhysicsPlugin;
use mcrs_minecraft_engine_world::tick_pace::tick_runs_normally;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_vanilla_item_entity::ItemEntitySet;

pub(crate) fn add_entity_motion(sub_app: &mut SubApp) {
    if !sub_app.is_plugin_added::<EntityMotionPlugin>() {
        sub_app.add_plugins(EntityMotionPlugin);
    }
    if !sub_app.is_plugin_added::<EntityMotionReplicationPlugin>() {
        sub_app.add_plugins(EntityMotionReplicationPlugin);
    }
}

pub(crate) fn build_dimension_mobs(_: &World, _: &DimensionBuild<'_>, sub_app: &mut SubApp) {
    if !sub_app.is_plugin_added::<EntityPhysicsPlugin>() {
        sub_app.add_plugins(EntityPhysicsPlugin);
    }
    add_entity_motion(sub_app);
    sub_app.configure_sets(FixedUpdate, BlockSimulationSet.before(DamageSet::Collect));
    sub_app.configure_sets(
        FixedUpdate,
        (
            MotionSteps,
            MobBrainSet::Prepare,
            MobBrainSet::Stage,
            ItemEntitySet,
        )
            .distributive_run_if(tick_runs_normally),
    );
}
