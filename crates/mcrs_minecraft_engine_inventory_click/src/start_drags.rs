use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use mcrs_minecraft_engine_inventory_menu::MenuOpened;

use crate::MenuDrag;

pub(crate) fn start_drags(opened: On<MenuOpened>, mut commands: Commands) {
    commands.entity(opened.menu).try_insert(MenuDrag::default());
}
