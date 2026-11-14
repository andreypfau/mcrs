use bevy_app::{App, Plugin};
use mcrs_minecraft_client_world::columns::ColumnStore;
use mcrs_minecraft_entity_visual::EntityTickPlugin;

use crate::network_entities::NetworkEntities;
use crate::receive;

/// Keeps the client entities that the server adds, and ticks them in `FixedUpdate` in
/// [`ClientEntityTick`](mcrs_minecraft_entity_visual::ClientEntityTick). Packets apply when they arrive, before the next tick.
pub struct ClientEntityPlugin;

impl Plugin for ClientEntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkEntities>()
            .add_plugins(EntityTickPlugin::<ColumnStore>::default());
        receive::add_observers(app);
    }
}
