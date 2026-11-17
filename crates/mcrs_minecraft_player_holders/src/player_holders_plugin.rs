use bevy_app::App;
use bevy_app::Plugin;
use mcrs_minecraft_engine_inventory_menu::MenuPlugin;

use crate::restore_holders::hold_new_player;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerHoldersPlugin;

impl Plugin for PlayerHoldersPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MenuPlugin>() {
            app.add_plugins(MenuPlugin);
        }
        app.add_observer(hold_new_player);
    }
}
