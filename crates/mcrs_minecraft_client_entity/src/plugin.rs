use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;

use crate::client_entity::NetworkEntities;
use crate::receive;
use crate::server_ticking::ServerTicking;
use crate::tick::{ClientEntityTick, consume_step, run_world_ticks, tick_entities};

/// Keeps the client entities that the server adds, and ticks them in `FixedUpdate` in
/// [`ClientEntityTick`]. Packets apply when they arrive, before the next tick.
pub struct ClientEntityPlugin;

impl Plugin for ClientEntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkEntities>()
            .init_resource::<ServerTicking>()
            .add_systems(
                FixedUpdate,
                (tick_entities, consume_step)
                    .chain()
                    .in_set(ClientEntityTick)
                    .run_if(run_world_ticks),
            );
        receive::add_observers(app);
    }
}
