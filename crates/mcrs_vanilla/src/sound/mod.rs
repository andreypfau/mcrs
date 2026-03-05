pub mod minecraft;

use mcrs_core::ResourceLocation;

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

#[derive(Clone, Copy, Debug)]
pub struct SoundType {
    pub volume: f32,
    pub pitch: f32,
    pub break_sound: ResourceLocation<&'static str>,
    pub step_sound: ResourceLocation<&'static str>,
    pub place_sound: ResourceLocation<&'static str>,
    pub hit_sound: ResourceLocation<&'static str>,
    pub fall_sound: ResourceLocation<&'static str>,
}

impl SoundType {
    pub const EMPTY: SoundType = Self::new(
        1.0,
        1.0,
        minecraft::EMPTY,
        minecraft::EMPTY,
        minecraft::EMPTY,
        minecraft::EMPTY,
        minecraft::EMPTY,
    );
    pub const WOOD: SoundType = Self::new(
        1.0,
        1.0,
        minecraft::WOOD_BREAK,
        minecraft::WOOD_STEP,
        minecraft::WOOD_PLACE,
        minecraft::WOOD_HIT,
        minecraft::WOOD_FALL,
    );
    pub const STONE: SoundType = Self::new(
        1.0,
        1.0,
        minecraft::STONE_BREAK,
        minecraft::STONE_STEP,
        minecraft::STONE_PLACE,
        minecraft::STONE_HIT,
        minecraft::STONE_FALL,
    );
    pub const GRASS: SoundType = Self::new(
        1.0,
        1.0,
        minecraft::EMPTY,
        minecraft::EMPTY,
        minecraft::EMPTY,
        minecraft::EMPTY,
        minecraft::EMPTY,
    );

    const fn new(
        volume: f32,
        pitch: f32,
        break_sound: ResourceLocation<&'static str>,
        step_sound: ResourceLocation<&'static str>,
        place_sound: ResourceLocation<&'static str>,
        hit_sound: ResourceLocation<&'static str>,
        fall_sound: ResourceLocation<&'static str>,
    ) -> Self {
        SoundType {
            volume,
            pitch,
            break_sound,
            step_sound,
            place_sound,
            hit_sound,
            fall_sound,
        }
    }
}
