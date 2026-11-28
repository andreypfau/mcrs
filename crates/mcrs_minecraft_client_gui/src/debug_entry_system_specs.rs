use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_resource::{ResourceLocation, rl};

use crate::debug_displayer::DebugScreenDisplayer;

pub const GROUP: ResourceLocation<'static> = rl!("minecraft:system");

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a debug screen refresh, at most ten times a second while the F3 overlay shows or when the stats log is due"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(5))]
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
