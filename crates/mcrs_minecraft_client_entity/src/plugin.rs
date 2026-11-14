use bevy_app::{App, FixedUpdate, Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_entity_visual::{ClientEntityTick, EntityLightSystems, EntityTickPlugin};

use crate::entity_light::{insert_entity_light, update_entity_light};
use crate::network_entities::NetworkEntities;
use crate::receive;

/// Keeps the client entities that the server adds, and ticks them in `FixedUpdate` in
/// [`ClientEntityTick`](mcrs_minecraft_entity_visual::ClientEntityTick). Packets apply when they arrive, before the next tick.
/// Writes the [`EntityLight`](mcrs_minecraft_entity_visual::EntityLight) of each entity from the
/// [`ColumnStore`]: after the tick, and in [`EntityLightSystems`] for an entity with no light yet.
pub struct ClientEntityPlugin;

impl Plugin for ClientEntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkEntities>()
            .add_plugins(EntityTickPlugin::<ColumnStore>::default())
            .add_systems(
                FixedUpdate,
                update_entity_light
                    .after(ClientEntityTick)
                    .run_if(resource_exists::<ColumnStore>),
            )
            .add_systems(
                PostUpdate,
                insert_entity_light
                    .in_set(EntityLightSystems)
                    .run_if(resource_exists::<ColumnStore>),
            );
        receive::add_observers(app);
    }
}
