//! `worldgen/carver/*.json` in full: the configuration surface the shared
//! carver engine has to express.

use serde::{Deserialize, Serialize};

use mcrs_minecraft_value_provider::{FloatProvider, HeightProvider, IntProvider};

fn one() -> FloatProvider {
    FloatProvider::Constant(1.0)
}

fn is_one(provider: &FloatProvider) -> bool {
    *provider == one()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::TypePath))]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum CarverConfig {
    Cave {
        probability: f32,
        y: HeightProvider,
        count: IntProvider,
        thickness: FloatProvider,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        weird_thickness_bias: bool,
        room_vertical_radius_multiplier: FloatProvider,
        horizontal_radius_multiplier: FloatProvider,
        vertical_radius_multiplier: FloatProvider,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        start_vertical_radius_multiplier: FloatProvider,
        floor_level: FloatProvider,
    },
    Canyon {
        probability: f32,
        y: HeightProvider,
        vertical_rotation: FloatProvider,
        shape: CanyonShape,
    },
    /// Beta's `MapGenCaves`. Nothing about it is configurable: its draws, its
    /// seed and its abort on water are the carver.
    BetaCave,
}

mcrs_minecraft_registry::dispatch! {
    CarverConfig, key = "type", registry = crate::keys::CarverType,
    {
        Cave => Cave,
        Canyon => Canyon,
    }
    extend { "mcrs:beta_cave" => BetaCave }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanyonShape {
    pub distance_factor: FloatProvider,
    pub thickness: FloatProvider,
    pub width_smoothness: i32,
    pub horizontal_radius_factor: FloatProvider,
    pub vertical_radius_default_factor: f32,
    pub vertical_radius_center_factor: f32,
    pub y_scale: FloatProvider,
}

impl CarverConfig {
    /// Whether this carver seeds a cave in the source chunk at all. The draw
    /// happens once per source, before any of the per-cave draws; a carver with
    /// no probability takes no such draw.
    pub fn probability(&self) -> Option<f32> {
        match *self {
            CarverConfig::Cave { probability, .. } | CarverConfig::Canyon { probability, .. } => {
                Some(probability)
            }
            CarverConfig::BetaCave => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::rl;
    use mcrs_minecraft_worldgen_testing::{read, round_trips};

    #[test]
    fn every_shipped_carver_parses_and_round_trips() {
        assert_eq!(round_trips::<CarverConfig>("carver"), 5);
    }

    #[test]
    fn an_absent_start_vertical_radius_multiplier_is_one() {
        let CarverConfig::Cave {
            start_vertical_radius_multiplier,
            ..
        } = read("carver", &rl!("minecraft:cave").to_arc())
        else {
            panic!("cave.json is a cave carver");
        };
        assert_eq!(
            start_vertical_radius_multiplier,
            FloatProvider::Constant(1.0)
        );
    }
}
