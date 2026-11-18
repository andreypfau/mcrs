use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_resource::ResourceLocation;

use crate::TextureRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct BackgroundPart {
    pub texture: ResourceLocation<'static>,
    pub source: TextureRect,
    pub destination: GuiPoint,
}

impl BackgroundPart {
    #[must_use]
    pub const fn new(
        texture: ResourceLocation<'static>,
        source: TextureRect,
        destination: GuiPoint,
    ) -> Self {
        Self {
            texture,
            source,
            destination,
        }
    }
}
