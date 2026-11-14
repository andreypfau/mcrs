use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;

/// The video option Improved Transparency: the far terrain fades into the sky and the clouds behind
/// it rather than into the fog colour. Off, the frame has no sky image and no cloud image.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, ExtractResource)]
#[extract_app(RenderApp)]
pub struct ImprovedTransparency(pub bool);
