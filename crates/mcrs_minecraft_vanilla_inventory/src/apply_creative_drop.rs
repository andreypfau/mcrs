use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_pose::Pose;

use crate::apply_creative_set::fits;
use crate::apply_creative_set::is_creative;
use crate::launch_drops::{eye_height_of, launch_from_player};

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
            Option<&Pose>,
            Option<&AttributeInstances>,
            Option<&GameProfile>,
        ),
        With<PlayerHolders>,
    >,
    table: Res<ItemTable>,
    mut commands: Commands,
) {
    let Ok((game_mode, transform, pose, attributes, profile)) = players.get(player) else {
        return;
    };
    if !is_creative(game_mode) || !fits(Some(&stack), &table) {
        return;
    }
    let entity = commands.spawn_stack(stack);
    let eye_height = eye_height_of(pose, attributes);
    launch_from_player(entity, transform, eye_height, profile, Cause::Throw);
}
