use bevy::asset::AssetId;
use bevy::math::UVec2;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;

#[derive(Resource, Clone, Copy, ExtractResource)]
#[extract_app(RenderApp)]
pub(crate) struct GuiImages {
    pub(crate) atlas: AssetId<Image>,
    pub(crate) glint: AssetId<Image>,
    pub(crate) glyph_origin: UVec2,
}
