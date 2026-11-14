// wgpu's `Buffer: Sync` proof chain is deeper than the default 128 frames, and
// `#[derive(Resource)]` walks all of it.
#![recursion_limit = "256"]

mod colour;
mod light_color_plugin;
mod light_volume_render_plugin;
mod volume;

pub use light_color_plugin::{HeldItemLights, LightColorPlugin};
pub use light_volume_render_plugin::LightVolumeRenderPlugin;
pub use volume::{VolumeCommand, VolumeQueue, VolumeSettings};
