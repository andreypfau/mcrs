use std::marker::PhantomData;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_physics::MobPublish;

use crate::publish_mobs::publish_mobs;

/// The tile events of the mobs of a dimension world whose rows have the mob column `M`, in the
/// [`MobPublish`] systems of the entity pass.
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
        app.add_systems(FixedUpdate, publish_mobs::<M>.in_set(MobPublish));
    }
}
