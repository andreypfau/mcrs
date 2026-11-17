use bevy_app::{App, Plugin};

pub(crate) fn require<By: Plugin, P: Plugin>(app: &App) {
    assert!(
        app.is_plugin_added::<P>(),
        "{} needs {} added before it",
        std::any::type_name::<By>(),
        std::any::type_name::<P>()
    );
}
