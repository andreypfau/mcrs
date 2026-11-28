use bevy::prelude::*;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_client::CreativeScreen;

use crate::creative_screen_shown::shows_creative_screen;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::marker_toggle,
        reason = "the creative inventory systems filter on the CreativeScreen marker, which changes only when the creative screen opens or closes"
    )
)]
pub(crate) fn mark_creative_screen(
    screen: Option<Res<Screen>>,
    players: Populated<(Entity, &Menus, Option<&Abilities>, Has<CreativeScreen>), With<Player>>,
    mut commands: Commands,
) {
    let screen = screen.as_deref().copied().unwrap_or_default();
    for (player, menus, abilities, marked) in &players {
        match (shows_creative_screen(screen, menus, abilities), marked) {
            (true, false) => {
                commands.entity(player).insert(CreativeScreen);
            }
            (false, true) => {
                commands.entity(player).remove::<CreativeScreen>();
            }
            (true, true) | (false, false) => {}
        }
    }
}
