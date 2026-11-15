use bevy_app::{App, FixedUpdate, Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_entity_visual::{ClientEntityTick, EntityLightSystems, EntityTickPlugin};

use crate::entity_light::write_entity_light;
use crate::network_entities::NetworkEntities;
use crate::orb_interpolation::step_orb_interpolations;
use crate::project_simulated_positions::project_simulated_positions;
use crate::receive;
use crate::velocity::tick_velocities;

/// Keeps the client entities that the server adds, and ticks them in `FixedUpdate` in
/// [`ClientEntityTick`](mcrs_minecraft_entity_visual::ClientEntityTick). Packets apply when they arrive, before the next tick.
/// Writes the [`EntityLight`](mcrs_minecraft_entity_visual::EntityLight) of each entity from the
/// [`ColumnStore`] in [`EntityLightSystems`]. The position of a simulated entity is in its
/// [`MotionState`](mcrs_minecraft_engine_entity_motion::MotionState); its
/// [`Motion`](mcrs_minecraft_entity_visual::Motion) follows after each tick and in `PostUpdate`.
pub struct ClientEntityPlugin;

impl Plugin for ClientEntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkEntities>()
            .add_plugins(EntityTickPlugin::<ColumnStore>::default())
            .add_systems(
                FixedUpdate,
                (
                    tick_velocities,
                    (step_orb_interpolations, project_simulated_positions).chain(),
                )
                    .after(ClientEntityTick),
            )
            .add_systems(
                PostUpdate,
                (
                    project_simulated_positions.before(EntityLightSystems),
                    write_entity_light
                        .in_set(EntityLightSystems)
                        .run_if(resource_exists::<ColumnStore>),
                ),
            );
        receive::add_observers(app);
    }
}
