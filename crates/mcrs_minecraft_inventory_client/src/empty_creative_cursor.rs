use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::CreativeScreen;
use crate::local_inventory;

pub(crate) fn empty_creative_cursor(
    closed: On<Remove<CreativeScreen>>,
    viewers: Query<(&PlayerHolders, &Menus)>,
    holders: Query<&Slots>,
    mut commands: Commands,
) {
    let Ok((player_holders, menus)) = viewers.get(closed.entity) else {
        return;
    };
    if !local_inventory::shows_inventory_menu(menus) {
        return;
    }
    let carried = (holders.get(player_holders.cursor).ok()).and_then(|slots| slots.get(0));
    if let Some(stack) = carried {
        commands.entity(stack).try_despawn();
    }
}
