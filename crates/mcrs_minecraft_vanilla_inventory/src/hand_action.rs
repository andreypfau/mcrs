use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_slots::OFF_HAND;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;

use crate::PlayerHolders;

const HAND_DROP_REQUEST: RequestId = RequestId(1);
pub(crate) const HAND_SWAP_REQUEST: RequestId = RequestId(2);

/// Makes the request of a drop key or of the hand swap of a player who is no spectator
/// (INV-SEL-004, INV-SEL-005). A drop from an empty hand and a swap of two empty slots request
/// nothing.
pub(crate) fn hand_action(
    received: On<Received<ServerboundPlayerAction>>,
    players: Query<(&Slots, &Selected, Option<&PlayerGameMode>), With<PlayerHolders>>,
    counts: Query<&Count>,
    mut commands: Commands,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let player = received.entity;
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
    let request = match packet.action {
        PlayerAction::DropAllItems | PlayerAction::DropItem => {
            let Some(stack) = held else {
                return;
            };
            let count = match packet.action {
                PlayerAction::DropItem => 1,
                _ => match counts.get(stack) {
                    Ok(count) => count.get(),
                    Err(_) => return,
                },
            };
            Request::new(HAND_DROP_REQUEST).drop(hand, count)
        }
        PlayerAction::SwapItemWithOffhand => {
            if held.is_none() && slots.get(usize::from(OFF_HAND)).is_none() {
                return;
            }
            let off_hand = SlotRef {
                holder: player,
                index: OFF_HAND,
            };
            Request::new(HAND_SWAP_REQUEST).swap(hand, off_hand)
        }
        _ => return,
    };
    commands.entity(player).request(request);
}
