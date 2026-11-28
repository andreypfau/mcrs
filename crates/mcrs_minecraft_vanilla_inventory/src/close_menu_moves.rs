use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_inventory_menu::MenuClosed;
use mcrs_minecraft_engine_inventory_menu::Menus;

use crate::apply_close::apply_close;
use crate::close_scope::CloseScope;

pub(crate) fn close_menu_moves(
    closed: On<MenuClosed>,
    menus: Query<&Menus>,
    mut commands: Commands,
) {
    let MenuClosed { viewer, menu } = *closed;
    let Ok(menus) = menus.get(viewer) else {
        return;
    };
    let scope = if menus.iter().next() == Some(menu) {
        CloseScope::InventoryMenu
    } else {
        CloseScope::OtherMenu
    };
    commands.run_system_cached_with(apply_close, (viewer, scope));
}
