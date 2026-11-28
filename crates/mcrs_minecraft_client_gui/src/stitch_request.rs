use bevy::math::UVec2;
use mcrs_minecraft_resource::ResourceLocation;

use crate::background_region::background_region;

#[derive(Clone, Copy)]
pub(crate) enum StitchRequest {
    Sprite(ResourceLocation<'static>),
    Background {
        texture: ResourceLocation<'static>,
        region: &'static str,
        extent: UVec2,
    },
}

impl StitchRequest {
    pub(crate) fn background(texture: ResourceLocation<'static>, extent: UVec2) -> Option<Self> {
        let region = background_region(texture)?;
        Some(Self::Background {
            texture,
            region,
            extent,
        })
    }

    pub(crate) const fn region(self) -> &'static str {
        match self {
            Self::Sprite(location) => location.path(),
            Self::Background { region, .. } => region,
        }
    }
}
