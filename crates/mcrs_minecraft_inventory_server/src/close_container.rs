use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;

use crate::InventoryInput;

pub(crate) fn close_container(
    received: On<Received<ServerboundContainerClose>>,
    mut inputs: MessageWriter<InventoryInput>,
) {
    if received.packet().is_none() {
        return;
    }
    inputs.write(InventoryInput::Close {
        player: received.entity,
    });
}
