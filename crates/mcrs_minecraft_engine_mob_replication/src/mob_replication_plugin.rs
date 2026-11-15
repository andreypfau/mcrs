use std::marker::PhantomData;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::OwnAttributesSet;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_attributes::AttributeChanged;
use mcrs_minecraft_engine_mob_attributes::RowAttributes;
use mcrs_minecraft_engine_physics::MobPublish;

use crate::publish_mob_attributes::publish_mob_attributes;
use crate::publish_mobs::publish_mobs;

/// The tile events of the mobs of a dimension world whose rows have the mob column `M`, in the
/// [`MobPublish`] systems of the entity pass, with the attribute instances of the mobs that the
/// [`AttributeChanged`] messages name.
#[derive(Debug)]
pub struct MobReplicationPlugin<M> {
    column: PhantomData<fn() -> M>,
}

impl<M> Clone for MobReplicationPlugin<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for MobReplicationPlugin<M> {}

impl<M> Default for MobReplicationPlugin<M> {
    fn default() -> Self {
        Self {
            column: PhantomData,
        }
    }
}

impl<M: Send + Sync + 'static> Plugin for MobReplicationPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileEvents>();
        app.init_resource::<RowAttributes>();
        app.add_message::<AttributeChanged>();
        // The take of the tile events sorts the subjects; the order only settles the ambiguity
        // check.
        app.configure_sets(FixedUpdate, OwnAttributesSet.before(MobPublish));
        app.add_systems(
            FixedUpdate,
            (
                publish_mobs::<M>,
                // The other systems of the set write other subjects, which the take of the tile
                // events sorts, or forget the instances of rows that left, whose events go.
                publish_mob_attributes::<M>
                    .run_if(on_message::<AttributeChanged>)
                    .ambiguous_with(MobPublish),
            )
                .chain()
                .in_set(MobPublish),
        );
    }
}
