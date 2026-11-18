use bevy::prelude::*;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_game_mode::Abilities;

use crate::creative_screen_shown::creative_screen_shown;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;

pub fn open_creative_screen(
    screen: Res<Screen>,
    player: Query<(&Menus, Option<&Abilities>), With<Player>>,
    content: Res<CreativeTabContent>,
    mut creative: ResMut<CreativeScreenState>,
    mut shown_before: Local<bool>,
) {
    let shown = creative_screen_shown(screen, player);
    if shown && !*shown_before {
        creative.open(&content);
    }
    *shown_before = shown;
}
