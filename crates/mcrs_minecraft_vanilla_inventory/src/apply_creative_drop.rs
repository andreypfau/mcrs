use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;

use crate::PlayerHolders;
use crate::apply_creative_set::fits;
use crate::apply_creative_set::is_creative;
use crate::launch_drops::launch_from_player;

/// Throws `stack` from a player in creative mode, when its count is not above its maximum stack
/// size (INV-CRE-003, IEN-SPN-007).
#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of the player that a throw needs"
)]
pub(crate) fn apply_creative_drop(
    In((player, stack)): In<(Entity, ItemStack)>,
    players: Query<
        (
            Option<&PlayerGameMode>,
            Option<&Transform>,
            Option<&GameProfile>,
        ),
        With<PlayerHolders>,
    >,
    table: Res<ItemTable>,
    mut commands: Commands,
) {
    let Ok((game_mode, transform, profile)) = players.get(player) else {
        return;
    };
    if !is_creative(game_mode) || !fits(Some(&stack), &table) {
        return;
    }
    let mut entity = commands.spawn_stack(stack);
    launch_from_player(&mut entity, transform, profile, Cause::Throw);
}
