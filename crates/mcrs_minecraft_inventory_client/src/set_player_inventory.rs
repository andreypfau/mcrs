use bevy_ecs::observer::On;
use bevy_ecs::system::Local;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetPlayerInventory;

use crate::local_inventory::LocalInventory;
use crate::local_inventory::StackNames;

pub(crate) fn set_player_inventory(
    received: On<Received<ClientboundSetPlayerInventory>>,
    mut inventory: LocalInventory,
    names: StackNames,
    mut bytes: Local<Vec<u8>>,
) {
    let (Some(packet), Some(names)) = (received.packet(), names.names()) else {
        return;
    };
    if let Ok(index) = u16::try_from(packet.slot.0) {
        let slot = SlotRef {
            holder: inventory.player(),
            index,
        };
        if let Err(error) = inventory.set(slot, &packet.contents, &names, &mut bytes) {
            tracing::warn!(%error, "a stack from the server has no value");
        }
    }
}
