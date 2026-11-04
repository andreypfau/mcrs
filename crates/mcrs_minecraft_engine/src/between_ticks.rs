//! What the host does between two ticks. Each step lets the coordinator act first and then the
//! muxer take the messages that the coordinator wrote, before the next step starts.

use bevy_app::App;
use bevy_ecs::world::World;

use crate::coordinator::dim;
use crate::coordinator::sub_app_builder;
use crate::muxer::dim_lifecycle;
use crate::muxer::dim_outputs;

/// Takes what the dimension worlds handed over in their last tick: the coordinator routes the
/// players that left, and the muxer sends the rest to the clients.
pub fn take_dim_outputs(world: &mut World) {
    dim::take_dim_outputs(world);
    dim_outputs::receive(world);
}

/// Builds a dimension world for each request of the host's `DimSpawnQueue`, then lets the muxer
/// take input for them.
pub fn drain_dim_spawn_queue(app: &mut App) {
    sub_app_builder::drain_dim_spawn_queue(app);
    dim_lifecycle::receive(app.world_mut());
}

/// Stops the dimension worlds of the `DimDespawnQueue`, then lets the muxer let go of what it
/// holds for them.
pub fn drain_dim_despawn_queue(app: &mut App) {
    dim::drain_dim_despawn_queue(app);
    dim_lifecycle::receive(app.world_mut());
}
