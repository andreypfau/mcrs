use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Java's `LevelBasedValue.CODEC`: a constant is a bare float and anything else
/// is the dispatched object.
#[derive(Debug, Clone, PartialEq)]
pub enum LevelBasedValue {
    Constant(f32),
    Clamped {
        value: Box<LevelBasedValue>,
        min: f32,
        max: f32,
    },
    Fraction {
        numerator: Box<LevelBasedValue>,
        denominator: Box<LevelBasedValue>,
    },
    LevelsSquared {
        added: f32,
    },
    Linear {
        base: f32,
        per_level_above_first: f32,
    },
    Exponent {
        base: Box<LevelBasedValue>,
        power: Box<LevelBasedValue>,
    },
    Lookup {
        values: Vec<f32>,
        fallback: Box<LevelBasedValue>,
    },
}

impl LevelBasedValue {
    pub fn calculate(&self, level: i32) -> f32 {
        match self {
            LevelBasedValue::Constant(value) => *value,
            LevelBasedValue::Clamped { value, min, max } => {
                value.calculate(level).clamp(*min, *max)
            }
            LevelBasedValue::Fraction {
                numerator,
                denominator,
            } => {
                let denominator = denominator.calculate(level);
                if denominator == 0.0 {
                    0.0
                } else {
                    numerator.calculate(level) / denominator
                }
            }
            LevelBasedValue::LevelsSquared { added } => (level * level) as f32 + added,
            LevelBasedValue::Linear {
                base,
                per_level_above_first,
            } => base + per_level_above_first * (level - 1) as f32,
            LevelBasedValue::Exponent { base, power } => {
                base.calculate(level).powf(power.calculate(level))
            }
            LevelBasedValue::Lookup { values, fallback } => {
                if level >= 1 && (level as usize) <= values.len() {
                    values[level as usize - 1]
                } else {
                    fallback.calculate(level)
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Self", deny_unknown_fields)]
enum DispatchedLevelBasedValue {
    Clamped {
        value: LevelBasedValue,
        min: f32,
        max: f32,
    },
    Fraction {
        numerator: LevelBasedValue,
        denominator: LevelBasedValue,
    },
    LevelsSquared {
        added: f32,
    },
    Linear {
        base: f32,
        per_level_above_first: f32,
    },
    Exponent {
        base: LevelBasedValue,
        power: LevelBasedValue,
    },
    Lookup {
        values: Vec<f32>,
        fallback: LevelBasedValue,
    },
}

mcrs_minecraft_registry::dispatch! {
    DispatchedLevelBasedValue, key = "type", registry = crate::keys::EnchantmentLevelBasedValueType,
    {
        Clamped => Clamped,
        Fraction => Fraction,
        LevelsSquared => LevelsSquared,
        Linear => Linear,
        Exponent => Exponent,
        Lookup => Lookup,
    }
}

impl From<DispatchedLevelBasedValue> for LevelBasedValue {
    fn from(value: DispatchedLevelBasedValue) -> Self {
        match value {
            DispatchedLevelBasedValue::Clamped { value, min, max } => LevelBasedValue::Clamped {
                value: Box::new(value),
                min,
                max,
            },
            DispatchedLevelBasedValue::Fraction {
                numerator,
                denominator,
            } => LevelBasedValue::Fraction {
                numerator: Box::new(numerator),
                denominator: Box::new(denominator),
            },
            DispatchedLevelBasedValue::LevelsSquared { added } => {
                LevelBasedValue::LevelsSquared { added }
            }
            DispatchedLevelBasedValue::Linear {
                base,
                per_level_above_first,
            } => LevelBasedValue::Linear {
                base,
                per_level_above_first,
            },
            DispatchedLevelBasedValue::Exponent { base, power } => LevelBasedValue::Exponent {
                base: Box::new(base),
                power: Box::new(power),
            },
            DispatchedLevelBasedValue::Lookup { values, fallback } => LevelBasedValue::Lookup {
                values,
                fallback: Box::new(fallback),
            },
        }
    }
}

impl<'de> Deserialize<'de> for LevelBasedValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = LevelBasedValue;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a level based value object")
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<LevelBasedValue, E> {
                Ok(LevelBasedValue::Constant(v as f32))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<LevelBasedValue, E> {
                Ok(LevelBasedValue::Constant(v as f32))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<LevelBasedValue, E> {
                Ok(LevelBasedValue::Constant(v as f32))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<LevelBasedValue, A::Error> {
                <DispatchedLevelBasedValue as Deserialize>::deserialize(
                    de::value::MapAccessDeserializer::new(map),
                )
                .map(LevelBasedValue::from)
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for LevelBasedValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let dispatched = match self {
            LevelBasedValue::Constant(value) => return s.serialize_f32(*value),
            LevelBasedValue::Clamped { value, min, max } => DispatchedLevelBasedValue::Clamped {
                value: (**value).clone(),
                min: *min,
                max: *max,
            },
            LevelBasedValue::Fraction {
                numerator,
                denominator,
            } => DispatchedLevelBasedValue::Fraction {
                numerator: (**numerator).clone(),
                denominator: (**denominator).clone(),
            },
            LevelBasedValue::LevelsSquared { added } => {
                DispatchedLevelBasedValue::LevelsSquared { added: *added }
            }
            LevelBasedValue::Linear {
                base,
                per_level_above_first,
            } => DispatchedLevelBasedValue::Linear {
                base: *base,
                per_level_above_first: *per_level_above_first,
            },
            LevelBasedValue::Exponent { base, power } => DispatchedLevelBasedValue::Exponent {
                base: (**base).clone(),
                power: (**power).clone(),
            },
            LevelBasedValue::Lookup { values, fallback } => DispatchedLevelBasedValue::Lookup {
                values: values.clone(),
                fallback: (**fallback).clone(),
            },
        };
        dispatched.serialize(s)
    }
}
