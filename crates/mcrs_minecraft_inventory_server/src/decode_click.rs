use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;

use crate::ClaimedSlot;
use crate::InventoryInput;

pub(crate) fn decode_click(
    received: On<Received<ServerboundContainerClick>>,
    mut inputs: MessageWriter<InventoryInput>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let changed = (packet.changed_slots.0.into_iter())
        .map(|(index, stack)| ClaimedSlot {
            index,
            view: SlotView::Hashed(stack),
        })
        .collect();
    inputs.write(InventoryInput::Click {
        player: received.entity,
        container_id: packet.container_id.0,
        state_id: packet.state_seqno.0,
        click: packet.click,
        changed,
        carried: SlotView::Hashed(packet.carried_item),
    });
}
