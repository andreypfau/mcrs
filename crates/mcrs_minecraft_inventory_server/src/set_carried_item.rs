use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCarriedItem;

use crate::InventoryInput;

pub(crate) fn set_carried_item(
    received: On<Received<ServerboundSetCarriedItem>>,
    mut inputs: MessageWriter<InventoryInput>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    inputs.write(InventoryInput::SelectSlot {
        player: received.entity,
        slot: packet.slot,
    });
}
