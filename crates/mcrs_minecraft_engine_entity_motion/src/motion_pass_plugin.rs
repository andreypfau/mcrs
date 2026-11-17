use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;

use crate::Movers;
use crate::StirredTiles;
use crate::movers::collect_movers;

#[derive(Clone, Copy, Debug, Default)]
pub struct MotionPassPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPass;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionRelease;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionSteps;

impl Plugin for MotionPassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StirredTiles>();
        app.init_resource::<Movers>();
        app.configure_sets(
            FixedUpdate,
            (MotionRelease, MotionSteps).chain().in_set(MotionPass),
        );
        app.add_systems(
            FixedUpdate,
            collect_movers.in_set(MotionPass).after(MotionSteps),
        );
    }
}
