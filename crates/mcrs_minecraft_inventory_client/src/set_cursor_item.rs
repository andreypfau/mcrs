use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;

use crate::CreativeScreen;
use crate::local_inventory::LocalInventory;
use crate::local_inventory::StackNames;

pub(crate) fn set_cursor_item(
    received: On<Received<ClientboundSetCursorItem>>,
    mut inventory: LocalInventory,
    names: StackNames,
    creative: Query<(), With<CreativeScreen>>,
    mut bytes: Local<Vec<u8>>,
) {
    if creative.contains(inventory.player()) {
        return;
    }
    let (Some(packet), Some(names)) = (received.packet(), names.names()) else {
        return;
    };
    if let Err(error) = inventory.set(inventory.cursor(), &packet.contents, &names, &mut bytes) {
        tracing::warn!(%error, "a stack from the server has no value");
    }
}
