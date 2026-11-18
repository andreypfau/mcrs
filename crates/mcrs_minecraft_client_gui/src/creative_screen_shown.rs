use bevy::prelude::*;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_client::shows_inventory_menu;

pub(crate) fn creative_screen_shown(
    screen: Option<Res<Screen>>,
    player: Query<(&Menus, Option<&Abilities>), With<Player>>,
) -> bool {
    player.single().is_ok_and(|(menus, abilities)| {
        screen.is_some_and(|screen| shows_creative_screen(*screen, menus, abilities))
    })
}

#[must_use]
pub(crate) fn shows_creative_screen(
    screen: Screen,
    menus: &Menus,
    abilities: Option<&Abilities>,
) -> bool {
    screen == Screen::Menu
        && shows_inventory_menu(menus)
        && abilities.is_some_and(|abilities| abilities.instant_build)
}
