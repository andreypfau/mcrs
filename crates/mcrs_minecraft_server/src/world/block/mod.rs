pub mod tnt;

pub struct MinecraftBlockPlugin;

impl bevy_app::Plugin for MinecraftBlockPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(tnt::TntBlockPlugin);
    }
}
