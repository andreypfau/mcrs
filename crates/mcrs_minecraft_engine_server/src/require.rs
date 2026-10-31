use bevy_app::{App, Plugin};

/// Panics unless `P` is built, so a missing member is named at build and not as a missing
/// resource at the first tick.
pub(crate) fn require<By: Plugin, P: Plugin>(app: &App) {
    assert!(
        app.is_plugin_added::<P>(),
        "{} needs {} added before it",
        std::any::type_name::<By>(),
        std::any::type_name::<P>()
    );
}
