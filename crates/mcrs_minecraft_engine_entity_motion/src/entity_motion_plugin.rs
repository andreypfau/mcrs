use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::resource_exists;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::voxel_update::BlockPlaced;

use crate::assign_ids::Assigned;
use crate::assign_ids::assign_ids;
use crate::departures::Departures;
use crate::departures::depart;
use crate::publish_motion::publish_motion;
use crate::step_motion::Movers;
use crate::step_motion::step_motion;
use crate::stirred_tiles::StirredTiles;
use crate::stirred_tiles::stir_tiles;

/// The motion of the moving entities of a dimension world that are not living. In
/// `DomainInput` it gives each new [`MotionState`](crate::MotionState) its id and keeps the tiles
/// of the changed blocks; in [`MotionPass`] it steps each entity; in [`MotionPublish`] it records
/// their tile events. The embedder inserts [`CellEntityIds`]; the entities move once
/// [`PhysicsTables`] exists.
#[derive(Clone, Copy, Debug, Default)]
pub struct EntityMotionPlugin;

/// The pass in `Simulate`: after it, each stepped entity holds the state and the
/// [`StepOutcome`](crate::StepOutcome) of this tick.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPass;

/// The tile events of the moving entities, in `Project`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPublish;

impl Plugin for EntityMotionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StirredTiles>();
        app.init_resource::<Assigned>();
        app.init_resource::<Movers>();
        app.init_resource::<Departures>();
        app.init_resource::<TileEvents>();
        app.add_message::<BlockPlaced>();
        app.add_observer(depart);
        app.configure_sets(FixedUpdate, MotionPass.in_set(TickSet::Simulate));
        // Both write the tile events of the tick.
        app.configure_sets(
            FixedUpdate,
            MotionPublish.in_set(TickSet::Project).after(MobPublish),
        );
        app.add_systems(
            FixedUpdate,
            (
                stir_tiles,
                assign_ids.run_if(resource_exists::<CellEntityIds>),
            )
                .in_set(TickSet::DomainInput),
        );
        app.add_systems(
            FixedUpdate,
            step_motion
                .run_if(resource_exists::<PhysicsTables>)
                .in_set(MotionPass),
        );
        app.add_systems(FixedUpdate, publish_motion.in_set(MotionPublish));
    }
}
