use bevy_ecs::message::MessageReader;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::Direction;
use mcrs_minecraft_protocol_game::player::PlayerAction;
use mcrs_minecraft_protocol_game::serverbound::ServerboundPlayerAction;
use mcrs_voxel::BlockPos;

use crate::HandInput;
use crate::click_purpose::ClickPurpose;
use crate::click_source::ClickSource;
use crate::outbox::Outbox;
use crate::outgoing::Outgoing;

#[expect(
    clippy::large_enum_variant,
    reason = "a plan lives on the stack for the one hand input that makes it"
)]
enum HandPlan {
    Ignore,
    Send(Option<Request>),
}

pub(crate) fn predict_hands(mut inputs: MessageReader<HandInput>, mut commands: Commands) {
    for input in inputs.read() {
        let input = *input;
        commands.queue(move |world: &mut World| predict_hand(world, input));
    }
}

fn predict_hand(world: &mut World, input: HandInput) {
    let Ok(HandPlan::Send(request)) = world.run_system_cached_with(plan_hand, input) else {
        return;
    };
    if let Some(request) = request {
        world.commands().entity(input.viewer).request(request);
        world.flush();
    }
    let action = match input.action {
        HandAction::DropOne => PlayerAction::DropItem,
        HandAction::DropStack => PlayerAction::DropAllItems,
        HandAction::SwapHands => PlayerAction::SwapItemWithOffhand,
    };
    if let Some(mut outbox) = world.get_resource_mut::<Outbox>() {
        outbox.0.push(Outgoing::Action(ServerboundPlayerAction {
            action,
            pos: BlockPos::new(0, 0, 0),
            direction: Direction::Down,
            sequence: VarInt(0),
        }));
    }
}

fn plan_hand(
    In(input): In<HandInput>,
    players: Query<(&Slots, &Selected, Option<&PlayerGameMode>), With<PlayerHolders>>,
    counts: Query<&Count>,
    source: Option<Res<ClickSource>>,
) -> HandPlan {
    let Ok((slots, selected, game_mode)) = players.get(input.viewer) else {
        return HandPlan::Ignore;
    };
    if game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Spectator) {
        return HandPlan::Ignore;
    }
    if input.action == HandAction::SwapHands {
        return HandPlan::Send(None);
    }
    let Some(source) = source else {
        return HandPlan::Send(None);
    };
    let id = RequestId {
        source: source.0,
        serial: ClickPurpose::Hand.serial(),
    };
    let held_count = (slots.get(usize::from(selected.0)))
        .and_then(|stack| counts.get(stack).ok())
        .map(|count| count.get());
    HandPlan::Send((input.action).request(input.viewer, slots, *selected, held_count, id))
}
