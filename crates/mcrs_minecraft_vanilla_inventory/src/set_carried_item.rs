use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCarriedItem;

use crate::PlayerHolders;

const HOTBAR_SLOTS: u16 = 9;

/// Makes a hotbar slot of the packet the selected slot, and ignores each other value
/// (INV-SEL-002).
pub(crate) fn set_carried_item(
    received: On<Received<ServerboundSetCarriedItem>>,
    mut players: Query<&mut Selected, With<PlayerHolders>>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    if packet.slot >= HOTBAR_SLOTS {
        return;
    }
    let Ok(slot) = u8::try_from(packet.slot) else {
        return;
    };
    if let Ok(mut selected) = players.get_mut(received.entity) {
        selected.set_if_neq(Selected(slot));
    }
}
