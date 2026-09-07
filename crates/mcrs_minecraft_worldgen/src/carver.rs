//! `worldgen/carver/*.json` in full: the configuration surface the shared
//! carver engine has to express.

use serde::{Deserialize, Serialize};

use crate::value_provider::{FloatProvider, HeightProvider, IntProvider};

fn one() -> FloatProvider {
    FloatProvider::Constant(1.0)
}

fn is_one(provider: &FloatProvider) -> bool {
    *provider == one()
}

fn is_false(value: &bool) -> bool {
    !*value
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
        #[serde(default, skip_serializing_if = "is_false")]
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
    /// happens once per source, before any of the per-cave draws.
    pub fn probability(&self) -> f32 {
        match *self {
            CarverConfig::Cave { probability, .. } | CarverConfig::Canyon { probability, .. } => {
                probability
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_provider::VerticalAnchor;
    use std::path::PathBuf;

    fn carver_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets/minecraft/worldgen/carver")
    }

    /// The reference reads these fields through `Codec.FLOAT`, so the numbers
    /// carry f32 precision and re-serializing widens them back to f64 with the
    /// digits that implies. Compare at the precision the codec actually has.
    fn at_f32_precision(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Number(n) => serde_json::json!(n.as_f64().unwrap() as f32 as f64),
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(at_f32_precision).collect())
            }
            serde_json::Value::Object(fields) => serde_json::Value::Object(
                fields
                    .iter()
                    .map(|(key, v)| (key.clone(), at_f32_precision(v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    #[test]
    fn every_shipped_carver_parses_and_round_trips() {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(carver_dir()).expect("carver dir must exist") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let config: CarverConfig = serde_json::from_slice(&bytes)
                .unwrap_or_else(|err| panic!("{} does not parse: {err}", path.display()));
            let written = serde_json::to_string(&config).unwrap();
            assert_eq!(
                serde_json::from_str::<CarverConfig>(&written).unwrap(),
                config,
                "{} does not survive a write and a re-read",
                path.display()
            );
            let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                at_f32_precision(&serde_json::to_value(&config).unwrap()),
                at_f32_precision(&raw),
                "{} does not round-trip",
                path.display()
            );
            names.push(path.file_stem().unwrap().to_string_lossy().into_owned());
        }
        names.sort();
        assert_eq!(
            names,
            ["canyon", "cave", "cave_extra_underground", "nether_cave"]
        );
    }

    #[test]
    fn the_cave_carver_reads_its_whole_surface() {
        let bytes = std::fs::read(carver_dir().join("cave.json")).unwrap();
        let CarverConfig::Cave {
            probability,
            y,
            count,
            thickness,
            weird_thickness_bias,
            start_vertical_radius_multiplier,
            floor_level,
            ..
        } = serde_json::from_slice(&bytes).unwrap()
        else {
            panic!("cave.json is a cave carver");
        };
        assert_eq!(probability, 0.15);
        assert_eq!(
            y,
            HeightProvider::Uniform {
                min_inclusive: VerticalAnchor::AboveBottom(8),
                max_inclusive: VerticalAnchor::Absolute(180),
            }
        );
        assert_eq!(
            count,
            IntProvider::VeryBiasedToBottom {
                min_inclusive: 0,
                max_inclusive: 14
            }
        );
        assert_eq!(
            thickness,
            FloatProvider::Trapezoid {
                min: 0.0,
                max: 3.0,
                plateau: 1.0
            }
        );
        assert!(weird_thickness_bias);
        // Absent from cave.json, and the reference defaults it to one.
        assert_eq!(
            start_vertical_radius_multiplier,
            FloatProvider::Constant(1.0)
        );
        assert_eq!(
            floor_level,
            FloatProvider::Uniform {
                min_inclusive: -1.0,
                max_exclusive: -0.4
            }
        );
    }

    /// The Nether's cave is the same carver with different values, including
    /// the bare-scalar form of every multiplier.
    #[test]
    fn the_nether_cave_is_the_cave_carver_with_scalars() {
        let bytes = std::fs::read(carver_dir().join("nether_cave.json")).unwrap();
        let CarverConfig::Cave {
            horizontal_radius_multiplier,
            vertical_radius_multiplier,
            room_vertical_radius_multiplier,
            start_vertical_radius_multiplier,
            floor_level,
            weird_thickness_bias,
            y,
            ..
        } = serde_json::from_slice(&bytes).unwrap()
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
            HeightProvider::Uniform {
                min_inclusive: VerticalAnchor::Absolute(0),
                max_inclusive: VerticalAnchor::BelowTop(1),
            }
        );
    }

    #[test]
    fn the_canyon_carver_reads_its_shape() {
        let bytes = std::fs::read(carver_dir().join("canyon.json")).unwrap();
        let CarverConfig::Canyon {
            probability,
            vertical_rotation,
            shape,
            ..
        } = serde_json::from_slice(&bytes).unwrap()
        else {
            panic!("canyon.json is a canyon carver");
        };
        assert_eq!(probability, 0.01);
        assert_eq!(
            vertical_rotation,
            FloatProvider::Uniform {
                min_inclusive: -0.125,
                max_exclusive: 0.125
            }
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
        let mut raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(carver_dir().join("canyon.json")).unwrap())
                .unwrap();
        raw["surprise"] = serde_json::json!(1);
        assert!(serde_json::from_value::<CarverConfig>(raw).is_err());
    }
}
