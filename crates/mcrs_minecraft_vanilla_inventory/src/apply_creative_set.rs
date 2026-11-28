use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::RequestPurpose;
use crate::VanillaInventoryRequests;
use crate::insert_stack::max_stack_size;

pub(crate) fn apply_creative_set(
    In((player, slot, stack)): In<(Entity, SlotRef, Option<ItemStack>)>,
    players: Query<Option<&PlayerGameMode>, With<PlayerHolders>>,
    table: Res<ItemTable>,
    requests: Option<Res<VanillaInventoryRequests>>,
    mut commands: Commands,
) {
    let Some(requests) = requests else {
        return;
    };
    let Ok(game_mode) = players.get(player) else {
        return;
    };
    if !is_creative(game_mode) || !fits(stack.as_ref(), &table) {
        return;
    }
    let count = stack.as_ref().map_or(0, |stack| stack.count.get());
    let spawned = stack.map(|stack| commands.spawn_stack(stack).id());
    let id = RequestId {
        source: requests.source,
        serial: RequestPurpose::Creative.serial(),
    };
    commands
        .entity(player)
        .request(Request::new(id).set(slot, spawned, count));
    if let Some(spawned) = spawned {
        commands.queue(move |world: &mut World| despawn_if_rejected(world, player, spawned, id));
    }
}

pub(crate) fn is_creative(game_mode: Option<&PlayerGameMode>) -> bool {
    game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Creative)
}

pub(crate) fn fits(stack: Option<&ItemStack>, table: &ItemTable) -> bool {
    stack.is_none_or(|stack| stack.count.get() <= max_stack_size(stack, table))
}

fn despawn_if_rejected(world: &mut World, player: Entity, stack: Entity, id: RequestId) {
    let rejected = (world.get_resource::<Messages<RequestResult>>())
        .and_then(|results| results.iter_current_update_messages().last())
        .is_some_and(|result| {
            result.holder == player && result.id == id && result.outcome.is_err()
        });
    if rejected {
        world.despawn(stack);
    }
}
