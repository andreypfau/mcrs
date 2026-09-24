pub mod minecraft;

use mcrs_minecraft_core::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundEvent {
    pub identifier: ResourceLocation<&'static str>,
    pub range: Option<f32>,
}

impl SoundEvent {
    pub const fn new(identifier: ResourceLocation<&'static str>, range: Option<f32>) -> Self {
        Self { identifier, range }
    }
}
