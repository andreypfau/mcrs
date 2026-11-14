use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_slots::OFF_HAND;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;

use crate::InventoryInput;
use crate::PlayerHolders;
use crate::request_ids::HAND_DROP_REQUEST;
use crate::request_ids::HAND_SWAP_REQUEST;

/// An action of the `player_action` packet on the hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandAction {
    /// The drop key: one item of the selected slot.
    DropOne,
    /// The drop key with the modifier: the whole stack of the selected slot.
    DropStack,
    /// The exchange of the selected slot and the off hand.
    SwapHands,
}

/// Writes the [`InventoryInput::Hand`] of each `player_action` packet with a drop key or the hand
/// swap.
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

/// Makes the request of a drop key or of the hand swap of a player who is no spectator
/// (INV-SEL-004, INV-SEL-005). A drop from an empty hand and a swap of two empty slots request
/// nothing.
pub(crate) fn apply_hand_action(
    InRef(input): InRef<InventoryInput>,
    players: Query<(&Slots, &Selected, Option<&PlayerGameMode>), With<PlayerHolders>>,
    counts: Query<&Count>,
    mut commands: Commands,
) {
    let &InventoryInput::Hand { player, action } = input else {
        return;
    };
    let Ok((slots, selected, game_mode)) = players.get(player) else {
        return;
    };
    if game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Spectator) {
        return;
    }
    let hand = SlotRef {
        holder: player,
        index: u16::from(selected.0),
    };
    let held = slots.get(usize::from(selected.0));
    let request = match action {
        HandAction::DropOne | HandAction::DropStack => {
            let Some(stack) = held else {
                return;
            };
            let count = match action {
                HandAction::DropOne => 1,
                _ => match counts.get(stack) {
                    Ok(count) => count.get(),
                    Err(_) => return,
                },
            };
            Request::new(HAND_DROP_REQUEST).drop(hand, count)
        }
        HandAction::SwapHands => {
            if held.is_none() && slots.get(usize::from(OFF_HAND)).is_none() {
                return;
            }
            let off_hand = SlotRef {
                holder: player,
                index: OFF_HAND,
            };
            Request::new(HAND_SWAP_REQUEST).swap(hand, off_hand)
        }
    };
    commands.entity(player).request(request);
}
