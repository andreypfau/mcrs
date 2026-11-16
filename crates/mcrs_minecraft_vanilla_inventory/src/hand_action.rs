use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_player::OFF_HAND;
use mcrs_minecraft_item_stack::Count;

use crate::PlayerHolders;
use crate::request_ids::HAND_DROP_REQUEST;
use crate::request_ids::HAND_SWAP_REQUEST;

/// An action of a player on its hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandAction {
    /// The drop key: one item of the selected slot.
    DropOne,
    /// The drop key with the modifier: the whole stack of the selected slot.
    DropStack,
    /// The exchange of the selected slot and the off hand.
    SwapHands,
}

/// Makes the request of a drop key or of the hand swap of a player who is no spectator
/// (INV-SEL-004, INV-SEL-005). A drop from an empty hand and a swap of two empty slots request
/// nothing.
pub(crate) fn apply_hand_action(
    In((player, action)): In<(Entity, HandAction)>,
    players: Query<(&Slots, &Selected, Option<&PlayerGameMode>), With<PlayerHolders>>,
    counts: Query<&Count>,
    mut commands: Commands,
) {
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
