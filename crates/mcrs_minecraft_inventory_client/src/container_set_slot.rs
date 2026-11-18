use bevy_ecs::observer::On;
use bevy_ecs::system::Local;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;

use crate::local_inventory::LocalInventory;
use crate::local_inventory::StackNames;

pub(crate) fn container_set_slot(
    received: On<Received<ClientboundContainerSetSlot>>,
    mut inventory: LocalInventory,
    names: StackNames,
    mut bytes: Local<Vec<u8>>,
) {
    let (Some(packet), Some(names)) = (received.packet(), names.names()) else {
        return;
    };
    let Some(menu) = inventory.menu_of(packet.container_id.0) else {
        return;
    };
    let Some(slot) = u16::try_from(packet.slot)
        .ok()
        .and_then(|index| inventory.entry(menu, index))
    else {
        return;
    };
    if let Err(error) = inventory.set(slot, &packet.item, &names, &mut bytes) {
        tracing::warn!(%error, "a stack from the server has no value");
    }
    inventory.set_state_id(menu, packet.state_seqno.0);
}
