use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::schedule::common_conditions::resource_exists;
use mcrs_minecraft_engine::OwnAttributesSet;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion::MotionRelease;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::assign_ids::Assigned;
use crate::assign_ids::assign_ids;
use crate::departures::Departures;
use crate::departures::depart;
use crate::hold_unseen::hold_unseen;
use crate::publish_motion::publish_motion;

/// The tile events of the moving entities of a dimension world, beside the
/// `EntityMotionPlugin` that moves them. It holds each new `MotionState`; in `MotionRelease` it
/// gives each held entity with an [`Appearance`](crate::Appearance) its id and releases it; in
/// [`MotionPublish`] it records the spawns, the moves of the `Movers` and the removals. The
/// embedder inserts [`CellEntityIds`].
#[derive(Clone, Copy, Debug, Default)]
pub struct EntityMotionReplicationPlugin;

/// The tile events of the moving entities, in `Project`.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MotionPublish;

impl Plugin for EntityMotionReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assigned>();
        app.init_resource::<Departures>();
        app.init_resource::<TileEvents>();
        app.add_observer(hold_unseen);
        app.add_observer(depart);
        // Both write the tile events of the tick.
        app.configure_sets(
            FixedUpdate,
            MotionPublish.in_set(TickSet::Project).after(MobPublish),
        );
        app.configure_sets(FixedUpdate, OwnAttributesSet.before(MobPublish));
        app.add_systems(
            FixedUpdate,
            assign_ids
                .run_if(resource_exists::<CellEntityIds>)
                .in_set(MotionRelease),
        );
        app.add_systems(FixedUpdate, publish_motion.in_set(MotionPublish));
    }
}
