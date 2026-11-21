use bevy_app::SubApp;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_physics::EntityMotionPlugin;
use mcrs_minecraft_engine_physics::EntityPhysicsPlugin;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;

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
}
