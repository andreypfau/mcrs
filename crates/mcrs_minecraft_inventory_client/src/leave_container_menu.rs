use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::clear_first_slot::clear_first_slot;

pub(crate) fn leave_container_menu(
    viewer: Entity,
    player_holders: &PlayerHolders,
    holders: &Query<&Slots>,
    commands: &mut Commands,
) {
    clear_first_slot(player_holders.cursor, holders, commands);
    commands.entity(viewer).close_menu();
}
