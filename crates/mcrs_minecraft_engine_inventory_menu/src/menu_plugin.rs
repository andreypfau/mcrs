use bevy_app::App;
use bevy_app::Plugin;

use crate::EntryPressed;
use crate::MenuClosed;
use crate::MenuOpened;
use crate::close_on_holder_removal::close_on_holder_removal;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MenuOpened>()
            .add_message::<MenuClosed>()
            .add_message::<EntryPressed>()
            .add_observer(close_on_holder_removal);
    }
}
