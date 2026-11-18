use bevy_ecs::observer::On;
use bevy_ecs::system::Local;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;

use crate::local_inventory::LocalInventory;
use crate::local_inventory::StackNames;

pub(crate) fn container_set_content(
    received: On<Received<ClientboundContainerSetContent>>,
    mut inventory: LocalInventory,
    names: StackNames,
    mut bytes: Local<Vec<u8>>,
    mut entries: Local<Vec<Option<SlotRef>>>,
) {
    let (Some(packet), Some(names)) = (received.packet(), names.names()) else {
        return;
    };
    let Some(menu) = inventory.menu_of(packet.container_id.0) else {
        return;
    };
    inventory.collect_entries(menu, packet.slot_data.len(), &mut entries);
    let mut first_error = None;
    let mut failed = 0_usize;
    for (slot, wire) in entries.iter().zip(&packet.slot_data) {
        let Some(slot) = *slot else {
            continue;
        };
        if let Err(error) = inventory.set(slot, wire, &names, &mut bytes) {
            first_error.get_or_insert(error);
            failed = failed.saturating_add(1);
        }
    }
    inventory.set_state_id(menu, packet.state_seqno.0);
    if inventory.is_open(menu)
        && let Err(error) =
            inventory.set(inventory.cursor(), &packet.carried_item, &names, &mut bytes)
    {
        first_error.get_or_insert(error);
        failed = failed.saturating_add(1);
    }
    if let Some(error) = first_error {
        tracing::warn!(%error, failed, "stacks from the server have no value");
    }
}
