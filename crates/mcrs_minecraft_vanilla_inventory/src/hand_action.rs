use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::RequestPurpose;
use crate::VanillaInventoryRequests;

pub(crate) fn apply_hand_action(
    In((player, action)): In<(Entity, HandAction)>,
    players: Query<(&Slots, &Selected, Option<&PlayerGameMode>), With<PlayerHolders>>,
    counts: Query<&Count>,
    requests: Option<Res<VanillaInventoryRequests>>,
    mut commands: Commands,
) {
    let Some(requests) = requests else {
        return;
    };
    let Ok((slots, selected, game_mode)) = players.get(player) else {
        return;
    };
    if game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Spectator) {
        return;
    }
    let purpose = match action {
        HandAction::DropOne | HandAction::DropStack => RequestPurpose::HandDrop,
        HandAction::SwapHands => RequestPurpose::HandSwap,
    };
    let id = RequestId {
        source: requests.source,
        serial: purpose.serial(),
    };
    let held_count = (slots.get(usize::from(selected.0)))
        .and_then(|stack| counts.get(stack).ok())
        .map(|count| count.get());
    let Some(request) = action.request(player, slots, *selected, held_count, id) else {
        return;
    };
    commands.entity(player).request(request);
}
