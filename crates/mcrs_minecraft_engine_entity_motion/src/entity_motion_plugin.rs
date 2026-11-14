use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::resource_exists;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;

use crate::Movers;
use crate::movers::collect_movers;
use crate::step_motion::step_motion;
use crate::stirred_tiles::StirredTiles;
use crate::stirred_tiles::stir_tiles;

/// The motion of the moving entities of a dimension world that are not living. In
/// `DomainInput` it keeps the tiles of the changed blocks; in [`MotionPass`] it steps each entity
/// with a [`MotionState`](crate::MotionState), a [`MotionKind`](crate::MotionKind) and no
/// [`MotionHold`](crate::MotionHold), then lists the [`Movers`]. The entities move once
/// [`PhysicsTables`] exists.
#[derive(Clone, Copy, Debug, Default)]
pub struct EntityMotionPlugin;

/// The pass in `Simulate`: after it, each stepped entity holds the state and the
/// [`StepOutcome`](crate::StepOutcome) of this tick, and [`Movers`] lists the entities that moved.
/// An entity that gets its motion before the pass, through commands, moves in the same tick.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPass;

/// The part of [`MotionPass`] before the step, where a plugin removes the
/// [`MotionHold`](crate::MotionHold) of the entities that may move in this tick. Its commands
/// apply before the step.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionRelease;

impl Plugin for EntityMotionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StirredTiles>();
        app.init_resource::<Movers>();
        app.init_resource::<BirthClock>();
        app.add_message::<BlockPlaced>();
        app.configure_sets(FixedUpdate, MotionPass.in_set(TickSet::Simulate));
        app.configure_sets(FixedUpdate, MotionRelease.in_set(MotionPass));
        app.add_systems(FixedUpdate, stir_tiles.in_set(TickSet::DomainInput));
        app.add_systems(
            FixedUpdate,
            (
                step_motion.run_if(resource_exists::<PhysicsTables>),
                collect_movers,
            )
                .chain()
                .in_set(MotionPass)
                .after(MotionRelease),
        );
    }
}
