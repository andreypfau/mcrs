use std::ops::RangeInclusive;

use mcrs_minecraft_core::codec::Validate;
use mcrs_minecraft_registries::{Named, Refs, Resolved, registry_value};
use serde::{Deserialize, Serialize};

use crate::Sound;

mcrs_minecraft_core::validated!(BlockSoundSet<Named>);

#[registry_value]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct BlockSoundSet<M: Refs = Resolved> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_sound: Option<Sound<M>>,
    #[serde(default = "unit", skip_serializing_if = "is_unit")]
    pub pitch: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place_sound: Option<Sound<M>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_sound: Option<Sound<M>>,
    #[serde(default = "unit", skip_serializing_if = "is_unit")]
    pub volume: f32,
}

impl BlockSoundSet {
    pub const EMPTY: BlockSoundSet = BlockSoundSet {
        break_sound: None,
        fall_sound: None,
        hit_sound: None,
        pitch: 1.0,
        place_sound: None,
        step_sound: None,
        volume: 1.0,
    };
    pub const VOLUME: RangeInclusive<f32> = 1.0e-5..=10.0;
    pub const PITCH: RangeInclusive<f32> = 1.0e-5..=2.0;
}

impl Default for BlockSoundSet {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl<M: Refs> Validate for BlockSoundSet<M> {
    fn validate(&self) -> Result<(), String> {
        within(&BlockSoundSet::VOLUME, "[1.0E-5;10.0]", self.volume)?;
        within(&BlockSoundSet::PITCH, "[1.0E-5;2.0]", self.pitch)
    }
}

fn within(range: &RangeInclusive<f32>, stated: &str, value: f32) -> Result<(), String> {
    if range.contains(&value) {
        Ok(())
    } else {
        Err(format!("Value must be within range {stated}: {value:?}"))
    }
}

fn unit() -> f32 {
    1.0
}

fn is_unit(value: &f32) -> bool {
    *value == 1.0
}
