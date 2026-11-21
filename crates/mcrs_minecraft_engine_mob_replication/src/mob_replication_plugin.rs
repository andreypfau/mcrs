use std::marker::PhantomData;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::OwnAttributesSet;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_mob_attributes::{RowAttributeChanged, RowAttributes};
use mcrs_minecraft_engine_physics::MobPublish;

use crate::publish_mob_attributes::publish_mob_attributes;
use crate::publish_mobs::publish_mobs;

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
        app.add_message::<RowAttributeChanged>();
        app.configure_sets(FixedUpdate, OwnAttributesSet.before(MobPublish));
        app.add_systems(
            FixedUpdate,
            (
                publish_mobs::<M>,
                publish_mob_attributes::<M>
                    .run_if(on_message::<RowAttributeChanged>)
                    .ambiguous_with(MobPublish),
            )
                .chain()
                .in_set(MobPublish),
        );
    }
}
