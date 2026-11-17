use bevy_app::App;
use bevy_app::Plugin;

use crate::end_drags::end_drags;
use crate::start_drags::start_drags;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuClickPlugin;

impl Plugin for MenuClickPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(start_drags).add_observer(end_drags);
    }
}
