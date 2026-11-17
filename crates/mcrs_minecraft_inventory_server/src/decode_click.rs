use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Local;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_item_stack::read_hashed;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::item::HashedStack;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_wire::Encode;

use crate::ClaimedSlot;
use crate::InventoryInput;
use crate::wire_names::WireNames;

pub(crate) fn decode_click(
    received: On<Received<ServerboundContainerClick>>,
    wire_names: WireNames,
    mut inputs: MessageWriter<InventoryInput>,
    mut bytes: Local<Vec<u8>>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let bytes = &mut *bytes;
    let names = wire_names.names();
    let changed = (packet.changed_slots.iter())
        .map(|(index, stack)| ClaimedSlot {
            index: *index,
            view: claimed_view(stack.as_ref(), &names, bytes),
        })
        .collect();
    inputs.write(InventoryInput::Click {
        player: received.entity,
        container_id: packet.container_id.0,
        state_id: packet.state_seqno.0,
        click: packet.click,
        changed,
        carried: claimed_view(packet.carried_item.as_ref(), &names, bytes),
    });
}

fn claimed_view(
    stack: Option<&HashedStack>,
    names: &RegistryNames<'_>,
    bytes: &mut Vec<u8>,
) -> SlotView {
    bytes.clear();
    if stack.encode(&mut *bytes).is_err() {
        return SlotView::Unknown;
    }
    match read_hashed(&mut bytes.as_slice(), names) {
        Ok(hashed) => SlotView::Hashed(hashed),
        Err(_) => SlotView::Unknown,
    }
}
