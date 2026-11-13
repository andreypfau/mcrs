use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCarriedItem;

use crate::InventoryInput;
use crate::PlayerHolders;

const HOTBAR_SLOTS: u16 = 9;

/// Writes the [`InventoryInput::SelectSlot`] of each `set_carried_item` packet.
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

/// Makes a hotbar slot the selected slot, and ignores each other value (INV-SEL-002).
pub(crate) fn apply_selected_slot(
    InRef(input): InRef<InventoryInput>,
    mut players: Query<&mut Selected, With<PlayerHolders>>,
) {
    let &InventoryInput::SelectSlot { player, slot } = input else {
        return;
    };
    if slot >= HOTBAR_SLOTS {
        return;
    }
    let Ok(slot) = u8::try_from(slot) else {
        return;
    };
    if let Ok(mut selected) = players.get_mut(player) {
        selected.set_if_neq(Selected(slot));
    }
}
