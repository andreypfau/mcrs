mod minecraft;

use mcrs_protocol::Ident;
use mcrs_registry::Holder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundEvent {
    sound_id: Ident<String>,
    range: Option<f32>,
}

impl SoundEvent {
    pub const fn new(sound_id: Ident<String>, range: Option<f32>) -> Self {
        Self { sound_id, range }
    }
}

impl<T> From<T> for SoundEvent
where
    T: Into<Ident<String>>,
{
    fn from(value: T) -> Self {
        SoundEvent::new(value.into(), None)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Music {
    sound: Holder<SoundEvent>,
    min_delay: i32,
    max_delay: i32,
    replace_current_music: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SoundType {
    pub volume: f32,
    pub pitch: f32,
    pub break_sound: Ident<&'static str>,
    pub step_sound: Ident<&'static str>,
    pub place_sound: Ident<&'static str>,
    pub hit_sound: Ident<&'static str>,
    pub fall_sound: Ident<&'static str>,
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
        break_sound: Ident<&'static str>,
        step_sound: Ident<&'static str>,
        place_sound: Ident<&'static str>,
        hit_sound: Ident<&'static str>,
        fall_sound: Ident<&'static str>,
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
