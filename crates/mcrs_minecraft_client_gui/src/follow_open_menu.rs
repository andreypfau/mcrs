use bevy::prelude::*;
use bevy::window::CursorOptions;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_engine_inventory_menu::Menus;

use crate::cursor_grab;

pub(crate) fn follow_open_menu(
    screen: Option<ResMut<Screen>>,
    player: Query<Ref<Menus>, (With<Player>, Changed<Menus>)>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let (Some(mut screen), Ok(menus)) = (screen, player.single()) else {
        return;
    };
    let Some(open) = menus.open() else {
        return;
    };
    let next = if menus.iter().next() == Some(open) {
        if menus.is_added() || *screen != Screen::Menu {
            return;
        }
        Screen::None
    } else {
        Screen::Menu
    };
    screen.set_if_neq(next);
    if let Ok(mut cursor) = cursor.single_mut() {
        cursor_grab::grab(&mut cursor, next == Screen::None);
    }
}
