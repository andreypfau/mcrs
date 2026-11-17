use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::observer::On;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_inventory_menu::MenuClosed;
use mcrs_minecraft_engine_inventory_menu::Menus;

use crate::MenuDrag;

pub(crate) fn end_drags(
    closed: On<MenuClosed>,
    viewers: Query<&Menus>,
    mut drags: Query<&mut MenuDrag>,
) {
    let Ok(menus) = viewers.get(closed.viewer) else {
        return;
    };
    for menu in menus.iter() {
        if let Ok(mut drag) = drags.get_mut(menu) {
            drag.set_if_neq(MenuDrag::default());
        }
    }
}
