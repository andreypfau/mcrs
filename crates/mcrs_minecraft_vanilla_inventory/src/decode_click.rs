use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Local;
use bevy_ecs::system::Res;
use mcrs_minecraft_inventory_click::Click;
use mcrs_minecraft_inventory_click::ClickKind;
use mcrs_minecraft_inventory_click::Clicked;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_stack::read_hashed;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::item::ContainerInput;
use mcrs_minecraft_protocol_game::item::HashedStack;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClick;
use mcrs_minecraft_wire::Encode;

use crate::ClaimedSlot;
use crate::InventoryInput;

/// Writes the [`InventoryInput::Click`] of each `container_click` packet.
pub(crate) fn decode_click(
    received: On<Received<ServerboundContainerClick>>,
    table: Res<ItemTable>,
    mut inputs: MessageWriter<InventoryInput>,
    mut bytes: Local<Vec<u8>>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let bytes = &mut *bytes;
    let changed = (packet.changed_slots.iter())
        .map(|(index, stack)| ClaimedSlot {
            index: *index,
            view: claimed_view(stack.as_ref(), &table, bytes),
        })
        .collect();
    inputs.write(InventoryInput::Click {
        player: received.entity,
        container_id: packet.container_id.0,
        state_id: packet.state_seqno.0,
        click: Click {
            kind: click_kind(packet.container_input),
            button: packet.button,
            clicked: Clicked::from(packet.slot_index),
        },
        changed,
        carried: claimed_view(packet.carried_item.as_ref(), &table, bytes),
    });
}

fn click_kind(input: ContainerInput) -> ClickKind {
    match input {
        ContainerInput::Pickup => ClickKind::PickUp,
        ContainerInput::QuickMove => ClickKind::QuickMove,
        ContainerInput::Swap => ClickKind::Swap,
        ContainerInput::Clone => ClickKind::Clone,
        ContainerInput::Throw => ClickKind::Throw,
        ContainerInput::QuickCraft => ClickKind::Drag,
        ContainerInput::PickupAll => ClickKind::PickUpAll,
    }
}

/// A hashed stack that the server cannot read gives a view that matches no stack.
// chisle: the two hashed stack models meet through the bytes of the wire; a direct conversion
// lifts it when the protocol holds the hashed stack of `mcrs_minecraft_item_stack`.
fn claimed_view(stack: Option<&HashedStack>, table: &ItemTable, bytes: &mut Vec<u8>) -> SlotView {
    bytes.clear();
    if stack.encode(&mut *bytes).is_err() {
        return SlotView::Unknown;
    }
    match read_hashed(&mut bytes.as_slice(), table) {
        Ok(hashed) => SlotView::Hashed(hashed),
        Err(_) => SlotView::Unknown,
    }
}
