use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use mcrs_minecraft_physics::BodyGrid;

use crate::CrowdCandidates;
use crate::Expired;
use crate::Movers;
use crate::StirredTiles;
use crate::body_grid_owner::own_body_grid;
use crate::collect_step_effects::collect_step_effects;
use crate::movers::collect_movers;
use crate::regrids::Regrids;

#[derive(Clone, Copy, Debug, Default)]
pub struct MotionPassPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPass;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionRelease;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionSteps;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionEffects;

impl Plugin for MotionPassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StirredTiles>();
        app.init_resource::<Movers>();
        app.init_resource::<BodyGrid>();
        app.init_resource::<Regrids>();
        app.init_resource::<CrowdCandidates>();
        app.init_resource::<Expired>();
        app.configure_sets(
            FixedUpdate,
            (MotionRelease, MotionSteps, MotionEffects)
                .chain()
                .in_set(MotionPass),
        );
        app.add_systems(
            FixedUpdate,
            (
                own_body_grid
                    .in_set(MotionPass)
                    .after(MotionRelease)
                    .before(MotionSteps),
                (collect_movers, collect_step_effects).in_set(MotionEffects),
            ),
        );
    }
}
