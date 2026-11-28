use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_resource::{ResourceLocation, rl};

use crate::debug_displayer::DebugScreenDisplayer;

pub const GROUP: ResourceLocation<'static> = rl!("minecraft:system");

pub fn display(
    mut displayer: ResMut<DebugScreenDisplayer>,
    adapter: Res<RenderAdapterInfo>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let backend = match adapter.driver_info.as_str() {
        "" => adapter.backend.to_string(),
        driver => format!("{} {driver}", adapter.backend),
    };
    displayer.add_to_group(
        GROUP,
        [
            format!(
                "Display: {}x{}",
                window.resolution.physical_width(),
                window.resolution.physical_height()
            ),
            adapter.name.clone(),
            backend,
        ],
    );
}
