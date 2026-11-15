use bevy_app::SubApp;
use mcrs_minecraft_engine_entity_motion_replication::EntityMotionReplicationPlugin;
use mcrs_minecraft_engine_physics::EntityMotionPlugin;

/// Adds the motion pass and its replication to a dimension world once, for the first of the
/// vanilla plugins of moving entities that the host adds.
pub(crate) fn add_entity_motion(sub_app: &mut SubApp) {
    if !sub_app.is_plugin_added::<EntityMotionPlugin>() {
        sub_app.add_plugins(EntityMotionPlugin);
    }
    if !sub_app.is_plugin_added::<EntityMotionReplicationPlugin>() {
        sub_app.add_plugins(EntityMotionReplicationPlugin);
    }
}
