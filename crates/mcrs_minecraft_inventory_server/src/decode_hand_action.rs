use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;

use crate::InventoryInput;

pub(crate) fn decode_hand_action(
    received: On<Received<ServerboundPlayerAction>>,
    mut inputs: MessageWriter<InventoryInput>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let action = match packet.action {
        PlayerAction::DropItem => HandAction::DropOne,
        PlayerAction::DropAllItems => HandAction::DropStack,
        PlayerAction::SwapItemWithOffhand => HandAction::SwapHands,
        _ => return,
    };
    inputs.write(InventoryInput::Hand {
        player: received.entity,
        action,
    });
}
