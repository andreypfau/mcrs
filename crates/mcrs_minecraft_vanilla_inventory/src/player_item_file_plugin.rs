use bevy_app::App;
use bevy_app::Plugin;

use crate::item_file_codec::register_ender_items;
use crate::item_file_codec::register_player_items;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerItemFilePlugin;

impl Plugin for PlayerItemFilePlugin {
    fn build(&self, app: &mut App) {
        register_ender_items(app);
        register_player_items(app);
    }
}
