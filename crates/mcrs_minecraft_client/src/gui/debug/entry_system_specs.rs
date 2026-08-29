use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_core::resource_location::ResourceLocation;

use super::{DebugEntryGroup, DebugScreenDisplayer};

pub const GROUP: DebugEntryGroup = ResourceLocation::new_static("minecraft:system");

/// Vanilla also names the Java runtime and the CPU model; neither has a source
/// here, and a native binary has no runtime to name in the first place.
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
