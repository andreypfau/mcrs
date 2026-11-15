use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;

use crate::Movers;
use crate::StirredTiles;
use crate::movers::collect_movers;

/// The resources, the sets and the collection of the [`Movers`] of the motion pass, without a
/// step: the host adds a system that runs [`MotionStep`](crate::MotionStep) over its blocks in
/// [`MotionSteps`], and places [`MotionPass`] in its tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct MotionPassPlugin;

/// The motion pass in `FixedUpdate`: after it, each stepped entity holds the state and the
/// [`StepOutcome`](crate::StepOutcome) of this tick, and [`Movers`] lists the entities that moved.
/// An entity that gets its motion before the pass, through commands, moves in the same tick.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPass;

/// The part of [`MotionPass`] before the step, where a plugin removes the
/// [`MotionHold`](crate::MotionHold) of the entities that may move in this tick. Its commands
/// apply before the step.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionRelease;

/// The part of [`MotionPass`] where the host steps its entities: after [`MotionRelease`] and
/// before the pass lists the [`Movers`]. The list is replaced in each tick, also when no step
/// runs.
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
