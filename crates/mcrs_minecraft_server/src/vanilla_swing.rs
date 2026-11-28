use bevy_app::{App, Plugin};
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_vanilla_swing::SwingPlugin;

pub struct VanillaSwingPlugin;

impl Plugin for VanillaSwingPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(SwingPlugin);
    }
}
