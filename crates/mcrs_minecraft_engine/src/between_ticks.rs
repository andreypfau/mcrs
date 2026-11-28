use bevy_app::App;
use bevy_ecs::world::World;

use crate::coordinator::dim;
use crate::coordinator::sub_app_builder;
use crate::muxer::dim_lifecycle;
use crate::muxer::dim_outputs;

pub fn take_dim_outputs(world: &mut World) {
    dim::take_dim_outputs(world);
    dim_outputs::receive(world);
}

pub fn drain_dim_spawn_queue(app: &mut App) {
    sub_app_builder::drain_dim_spawn_queue(app);
    dim_lifecycle::receive(app.world_mut());
}

pub fn drain_dim_despawn_queue(app: &mut App) {
    dim::drain_dim_despawn_queue(app);
    dim_lifecycle::receive(app.world_mut());
}
