use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;
use mcrs_minecraft_fog::FogRamp;

/// The fog that the terrain and the entities mix in: the colour `color` in image values, the ramp
/// of the spherical distance and the ramp of the cylindrical distance, in blocks from the camera.
/// The default draws no fog.
#[derive(Resource, Clone, Copy, Debug, PartialEq, ExtractResource)]
#[extract_app(RenderApp)]
pub struct WorldFog {
    pub color: [f32; 3],
    pub environmental: FogRamp,
    pub render_distance: FogRamp,
}

impl Default for WorldFog {
    fn default() -> Self {
        Self {
            color: [0.0; 3],
            environmental: FogRamp::NONE,
            render_distance: FogRamp::NONE,
        }
    }
}
