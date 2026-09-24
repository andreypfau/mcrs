//! `worldgen/carver/*.json` in full: the configuration surface the shared
//! carver engine has to express.

use serde::{Deserialize, Serialize};

use mcrs_minecraft_core::value_provider::{FloatProvider, HeightProvider, IntProvider};

fn one() -> FloatProvider {
    FloatProvider::Constant(1.0)
}

fn is_one(provider: &FloatProvider) -> bool {
    *provider == one()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::TypePath))]
#[serde(tag = "type", deny_unknown_fields)]
pub enum CarverConfig {
    #[serde(rename = "minecraft:cave")]
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
    #[serde(rename = "minecraft:canyon")]
    Canyon {
        probability: f32,
        y: HeightProvider,
        vertical_rotation: FloatProvider,
        shape: CanyonShape,
    },
    /// Beta's `MapGenCaves`. Nothing about it is configurable: its draws, its
    /// seed and its abort on water are the carver.
    #[serde(rename = "mcrs:beta_cave")]
    BetaCave,
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
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_core::value_provider::{
        DispatchedFloatProvider, DispatchedHeightProvider, DispatchedIntProvider, VerticalAnchor,
    };
    use mcrs_minecraft_worldgen_testing::{read, round_trips};

    fn carver<T: serde::de::DeserializeOwned>(name: &str) -> T {
        read("carver", &ResourceLocation::minecraft(name))
    }

    #[test]
    fn every_shipped_carver_parses_and_round_trips() {
        assert_eq!(round_trips::<CarverConfig>("carver"), 5);
    }

    #[test]
    fn the_cave_carver_reads_its_whole_surface() {
        let CarverConfig::Cave {
            probability,
            y,
            count,
            thickness,
            weird_thickness_bias,
            start_vertical_radius_multiplier,
            floor_level,
            ..
        } = carver("cave")
        else {
            panic!("cave.json is a cave carver");
        };
        assert_eq!(probability, 0.15);
        assert_eq!(
            y,
            HeightProvider::Dispatched(DispatchedHeightProvider::Uniform {
                min_inclusive: VerticalAnchor::AboveBottom(8),
                max_inclusive: VerticalAnchor::Absolute(180),
            })
        );
        assert_eq!(
            count,
            IntProvider::Dispatched(DispatchedIntProvider::VeryBiasedToBottom {
                min_inclusive: 0,
                max_inclusive: 14
            })
        );
        assert_eq!(
            thickness,
            FloatProvider::Dispatched(DispatchedFloatProvider::Trapezoid {
                min: 0.0,
                max: 3.0,
                plateau: 1.0
            })
        );
        assert!(weird_thickness_bias);
        // Absent from cave.json, and the reference defaults it to one.
        assert_eq!(
            start_vertical_radius_multiplier,
            FloatProvider::Constant(1.0)
        );
        assert_eq!(
            floor_level,
            FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
                min_inclusive: -1.0,
                max_exclusive: -0.4
            })
        );
    }

    /// The Nether's cave is the same carver with different values, including
    /// the bare-scalar form of every multiplier.
    #[test]
    fn the_nether_cave_is_the_cave_carver_with_scalars() {
        let CarverConfig::Cave {
            horizontal_radius_multiplier,
            vertical_radius_multiplier,
            room_vertical_radius_multiplier,
            start_vertical_radius_multiplier,
            floor_level,
            weird_thickness_bias,
            y,
            ..
        } = carver("nether_cave")
        else {
            panic!("nether_cave.json is a cave carver");
        };
        assert_eq!(horizontal_radius_multiplier, FloatProvider::Constant(1.0));
        assert_eq!(vertical_radius_multiplier, FloatProvider::Constant(1.0));
        assert_eq!(
            room_vertical_radius_multiplier,
            FloatProvider::Constant(0.5)
        );
        assert_eq!(
            start_vertical_radius_multiplier,
            FloatProvider::Constant(5.0)
        );
        assert_eq!(floor_level, FloatProvider::Constant(-0.7));
        assert!(!weird_thickness_bias);
        assert_eq!(
            y,
            HeightProvider::Dispatched(DispatchedHeightProvider::Uniform {
                min_inclusive: VerticalAnchor::Absolute(0),
                max_inclusive: VerticalAnchor::BelowTop(1),
            })
        );
    }

    #[test]
    fn the_canyon_carver_reads_its_shape() {
        let CarverConfig::Canyon {
            probability,
            vertical_rotation,
            shape,
            ..
        } = carver("canyon")
        else {
            panic!("canyon.json is a canyon carver");
        };
        assert_eq!(probability, 0.01);
        assert_eq!(
            vertical_rotation,
            FloatProvider::Dispatched(DispatchedFloatProvider::Uniform {
                min_inclusive: -0.125,
                max_exclusive: 0.125
            })
        );
        assert_eq!(shape.width_smoothness, 3);
        assert_eq!(shape.vertical_radius_default_factor, 1.0);
        assert_eq!(shape.vertical_radius_center_factor, 0.0);
        assert_eq!(shape.y_scale, FloatProvider::Constant(3.0));
    }

    #[test]
    fn an_unknown_carver_type_is_a_load_error() {
        assert!(serde_json::from_str::<CarverConfig>(r#"{"type":"minecraft:ravine"}"#).is_err());
    }

    #[test]
    fn an_unknown_field_is_a_load_error() {
        let mut raw: serde_json::Value = carver("canyon");
        raw["surprise"] = serde_json::json!(1);
        assert!(serde_json::from_value::<CarverConfig>(raw).is_err());
    }
}
