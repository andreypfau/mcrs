use bevy_app::App;
use bevy_app::Plugin;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerItemFilePlugin;

impl Plugin for PlayerItemFilePlugin {
    fn build(&self, app: &mut App) {
        crate::persist::register_player(app);
    }
}
