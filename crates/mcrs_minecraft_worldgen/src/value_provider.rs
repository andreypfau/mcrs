//! Java's `IntProviders`, `FloatProviders` and `HeightProvider` codecs, in the
//! forms the shipped worldgen registries carry, with the sampling the
//! generators need. An unnamed form is a datapack this build does not
//! understand, so it is a load error rather than a value to guess at.

use std::fmt;

use mcrs_minecraft_random::Random;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Where a vertical anchor sits, given the dimension's own extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightContext {
    pub min_y: i32,
    pub depth: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl VerticalAnchor {
    pub fn resolve_y(self, context: HeightContext) -> i32 {
        match self {
            VerticalAnchor::Absolute(y) => y,
            VerticalAnchor::AboveBottom(offset) => context.min_y + offset,
            VerticalAnchor::BelowTop(offset) => context.min_y + context.depth - 1 - offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntProvider {
    Constant(i32),
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    VeryBiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

impl IntProvider {
    pub fn sample<R: Random>(self, rng: &mut R) -> i32 {
        match self {
            IntProvider::Constant(value) => value,
            IntProvider::Uniform {
                min_inclusive,
                max_inclusive,
            } => rng.next_i32_bound(max_inclusive - min_inclusive + 1) + min_inclusive,
            IntProvider::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
            } => {
                let span = rng.next_i32_bound(max_inclusive - min_inclusive + 1) + 1;
                let span = rng.next_i32_bound(span) + 1;
                min_inclusive + rng.next_i32_bound(span)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatProvider {
    Constant(f32),
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    Trapezoid {
        min: f32,
        max: f32,
        plateau: f32,
    },
}

impl FloatProvider {
    pub fn sample<R: Random>(self, rng: &mut R) -> f32 {
        match self {
            FloatProvider::Constant(value) => value,
            FloatProvider::Uniform {
                min_inclusive,
                max_exclusive,
            } => rng.next_f32() * (max_exclusive - min_inclusive) + min_inclusive,
            FloatProvider::Trapezoid { min, max, plateau } => {
                let range = max - min;
                let ramp = (range - plateau) / 2.0;
                min + rng.next_f32() * (range - ramp) + rng.next_f32() * ramp
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeightProvider {
    Constant(VerticalAnchor),
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
}

impl HeightProvider {
    pub fn sample<R: Random>(self, rng: &mut R, context: HeightContext) -> i32 {
        match self {
            HeightProvider::Constant(anchor) => anchor.resolve_y(context),
            HeightProvider::Uniform {
                min_inclusive,
                max_inclusive,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if min > max {
                    return min;
                }
                rng.next_i32_bound(max - min + 1) + min
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum DispatchedIntProvider {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:very_biased_to_bottom")]
    VeryBiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum DispatchedFloatProvider {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid { min: f32, max: f32, plateau: f32 },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum DispatchedHeightProvider {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
}

impl<'de> Deserialize<'de> for IntProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = IntProvider;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an integer or an int provider object")
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<IntProvider, E> {
                i32::try_from(v)
                    .map(IntProvider::Constant)
                    .map_err(|_| E::custom(format!("int provider constant {v} is out of range")))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<IntProvider, E> {
                i32::try_from(v)
                    .map(IntProvider::Constant)
                    .map_err(|_| E::custom(format!("int provider constant {v} is out of range")))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<IntProvider, A::Error> {
                Ok(
                    match DispatchedIntProvider::deserialize(
                        de::value::MapAccessDeserializer::new(map),
                    )? {
                        DispatchedIntProvider::Uniform {
                            min_inclusive,
                            max_inclusive,
                        } => IntProvider::Uniform {
                            min_inclusive,
                            max_inclusive,
                        },
                        DispatchedIntProvider::VeryBiasedToBottom {
                            min_inclusive,
                            max_inclusive,
                        } => IntProvider::VeryBiasedToBottom {
                            min_inclusive,
                            max_inclusive,
                        },
                    },
                )
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for IntProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            IntProvider::Constant(value) => s.serialize_i32(value),
            IntProvider::Uniform {
                min_inclusive,
                max_inclusive,
            } => DispatchedIntProvider::Uniform {
                min_inclusive,
                max_inclusive,
            }
            .serialize(s),
            IntProvider::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
            } => DispatchedIntProvider::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
            }
            .serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for FloatProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = FloatProvider;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a float provider object")
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<FloatProvider, E> {
                Ok(FloatProvider::Constant(v as f32))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<FloatProvider, E> {
                Ok(FloatProvider::Constant(v as f32))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<FloatProvider, E> {
                Ok(FloatProvider::Constant(v as f32))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<FloatProvider, A::Error> {
                Ok(
                    match DispatchedFloatProvider::deserialize(
                        de::value::MapAccessDeserializer::new(map),
                    )? {
                        DispatchedFloatProvider::Uniform {
                            min_inclusive,
                            max_exclusive,
                        } => FloatProvider::Uniform {
                            min_inclusive,
                            max_exclusive,
                        },
                        DispatchedFloatProvider::Trapezoid { min, max, plateau } => {
                            FloatProvider::Trapezoid { min, max, plateau }
                        }
                    },
                )
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for FloatProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            FloatProvider::Constant(value) => s.serialize_f32(value),
            FloatProvider::Uniform {
                min_inclusive,
                max_exclusive,
            } => DispatchedFloatProvider::Uniform {
                min_inclusive,
                max_exclusive,
            }
            .serialize(s),
            FloatProvider::Trapezoid { min, max, plateau } => {
                DispatchedFloatProvider::Trapezoid { min, max, plateau }.serialize(s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for HeightProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        if value.get("type").is_some() {
            return match DispatchedHeightProvider::deserialize(value).map_err(de::Error::custom)? {
                DispatchedHeightProvider::Uniform {
                    min_inclusive,
                    max_inclusive,
                } => Ok(HeightProvider::Uniform {
                    min_inclusive,
                    max_inclusive,
                }),
            };
        }
        VerticalAnchor::deserialize(value)
            .map(HeightProvider::Constant)
            .map_err(de::Error::custom)
    }
}

impl Serialize for HeightProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            HeightProvider::Constant(anchor) => anchor.serialize(s),
            HeightProvider::Uniform {
                min_inclusive,
                max_inclusive,
            } => DispatchedHeightProvider::Uniform {
                min_inclusive,
                max_inclusive,
            }
            .serialize(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_random::legacy::LegacyRandom;

    fn round_trip<T>(json: &str) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + fmt::Debug,
    {
        let parsed: T = serde_json::from_str(json).expect("parses");
        let written = serde_json::to_string(&parsed).expect("writes");
        let again: T = serde_json::from_str(&written).expect("reparses");
        assert_eq!(parsed, again, "round trip changed the value: {written}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            serde_json::from_str::<serde_json::Value>(&written).unwrap(),
            "round trip changed the json"
        );
        parsed
    }

    #[test]
    fn a_bare_number_is_a_constant() {
        assert_eq!(round_trip::<IntProvider>("7"), IntProvider::Constant(7));
        assert_eq!(
            round_trip::<FloatProvider>("-0.7"),
            FloatProvider::Constant(-0.7)
        );
    }

    #[test]
    fn the_carver_registry_forms_round_trip() {
        assert_eq!(
            round_trip::<IntProvider>(
                r#"{"type":"minecraft:very_biased_to_bottom","min_inclusive":0,"max_inclusive":14}"#
            ),
            IntProvider::VeryBiasedToBottom {
                min_inclusive: 0,
                max_inclusive: 14
            }
        );
        assert_eq!(
            round_trip::<FloatProvider>(
                r#"{"type":"minecraft:trapezoid","min":0.0,"max":3.0,"plateau":1.0}"#
            ),
            FloatProvider::Trapezoid {
                min: 0.0,
                max: 3.0,
                plateau: 1.0
            }
        );
        assert_eq!(
            round_trip::<FloatProvider>(
                r#"{"type":"minecraft:uniform","min_inclusive":0.7,"max_exclusive":1.4}"#
            ),
            FloatProvider::Uniform {
                min_inclusive: 0.7,
                max_exclusive: 1.4
            }
        );
        assert_eq!(
            round_trip::<HeightProvider>(
                r#"{"type":"minecraft:uniform","min_inclusive":{"above_bottom":8},"max_inclusive":{"absolute":180}}"#
            ),
            HeightProvider::Uniform {
                min_inclusive: VerticalAnchor::AboveBottom(8),
                max_inclusive: VerticalAnchor::Absolute(180)
            }
        );
        assert_eq!(
            round_trip::<HeightProvider>(r#"{"below_top":1}"#),
            HeightProvider::Constant(VerticalAnchor::BelowTop(1))
        );
    }

    #[test]
    fn an_unnamed_form_is_a_load_error() {
        assert!(serde_json::from_str::<IntProvider>(r#"{"type":"minecraft:clamped"}"#).is_err());
        assert!(
            serde_json::from_str::<FloatProvider>(
                r#"{"type":"minecraft:uniform","min_inclusive":0.0,"max_exclusive":1.0,"extra":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn an_anchor_resolves_against_the_dimension_extent() {
        let overworld = HeightContext {
            min_y: -64,
            depth: 384,
        };
        assert_eq!(VerticalAnchor::Absolute(180).resolve_y(overworld), 180);
        assert_eq!(VerticalAnchor::AboveBottom(8).resolve_y(overworld), -56);
        assert_eq!(VerticalAnchor::BelowTop(1).resolve_y(overworld), 318);
    }

    /// Every provider draws exactly the values Java's own `sample` does, in the
    /// same order, so a shared seed has to produce the same numbers.
    #[test]
    fn sampling_matches_the_reference_draw_order() {
        let mut rng = LegacyRandom::new(42);
        let mut reference = LegacyRandom::new(42);

        let uniform_int = IntProvider::Uniform {
            min_inclusive: 3,
            max_inclusive: 9,
        };
        assert_eq!(
            uniform_int.sample(&mut rng),
            reference.next_i32_bound(9 - 3 + 1) + 3
        );

        let very_biased = IntProvider::VeryBiasedToBottom {
            min_inclusive: 0,
            max_inclusive: 14,
        };
        let expected = {
            let a = reference.next_i32_bound(15) + 1;
            let b = reference.next_i32_bound(a) + 1;
            reference.next_i32_bound(b)
        };
        assert_eq!(very_biased.sample(&mut rng), expected);

        let trapezoid = FloatProvider::Trapezoid {
            min: 0.0,
            max: 3.0,
            plateau: 1.0,
        };
        let expected = {
            let range = 3.0f32;
            let ramp = (range - 1.0) / 2.0;
            reference.next_f32() * (range - ramp) + reference.next_f32() * ramp
        };
        assert_eq!(trapezoid.sample(&mut rng), expected);
    }

    #[test]
    fn a_very_biased_sample_stays_in_range_and_leans_low() {
        let provider = IntProvider::VeryBiasedToBottom {
            min_inclusive: 0,
            max_inclusive: 14,
        };
        let mut rng = LegacyRandom::new(7);
        let mut zeroes = 0;
        for _ in 0..2000 {
            let value = provider.sample(&mut rng);
            assert!((0..=14).contains(&value), "{value} out of range");
            if value == 0 {
                zeroes += 1;
            }
        }
        assert!(zeroes > 600, "only {zeroes} zeroes of 2000");
    }
}
