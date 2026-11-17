use mcrs_minecraft_numeric::{clamp_f32, pow_f32};
use mcrs_minecraft_registries::dispatch;
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq)]
pub enum LevelBasedValue {
    Constant(f32),
    Dispatched(DispatchedLevelBasedValue),
}

impl LevelBasedValue {
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "toF32 of an i32 is the nearest f32"
    )]
    pub fn calculate(&self, level: i32) -> f32 {
        use DispatchedLevelBasedValue::*;
        let dispatched = match self {
            LevelBasedValue::Constant(value) => return *value,
            LevelBasedValue::Dispatched(dispatched) => dispatched,
        };
        match dispatched {
            Clamped { value, min, max } => clamp_f32(value.calculate(level), *min, *max),
            Fraction {
                numerator,
                denominator,
            } => {
                let denominator = denominator.calculate(level);
                match denominator == 0.0 {
                    true => 0.0,
                    false => numerator.calculate(level) / denominator,
                }
            }
            LevelsSquared { added } => level.wrapping_mul(level) as f32 + added,
            Linear {
                base,
                per_level_above_first,
            } => base + per_level_above_first * level.wrapping_sub(1) as f32,
            Exponent { base, power } => pow_f32(base.calculate(level), power.calculate(level)),
            Lookup { values, fallback } => usize::try_from(level)
                .ok()
                .and_then(|level| level.checked_sub(1))
                .and_then(|at| values.get(at).copied())
                .unwrap_or_else(|| fallback.calculate(level)),
        }
    }
}

#[dispatch(registry = crate::enchantment_level_based_value_type_keys::EnchantmentLevelBasedValueType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum DispatchedLevelBasedValue {
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
                .map(LevelBasedValue::Dispatched)
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for LevelBasedValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            LevelBasedValue::Constant(value) => s.serialize_f32(*value),
            LevelBasedValue::Dispatched(dispatched) => dispatched.serialize(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispatched(value: DispatchedLevelBasedValue) -> LevelBasedValue {
        LevelBasedValue::Dispatched(value)
    }

    fn boxed(value: f32) -> Box<LevelBasedValue> {
        Box::new(LevelBasedValue::Constant(value))
    }

    #[test]
    fn loot_num_021_each_type_gives_its_value() {
        use DispatchedLevelBasedValue::*;
        let linear = dispatched(Linear {
            base: 1.0,
            per_level_above_first: 2.0,
        });
        let squared = dispatched(LevelsSquared { added: 1.0 });
        let clamped = |value: f32| {
            dispatched(Clamped {
                value: boxed(value),
                min: 0.0,
                max: 2.0,
            })
        };
        let fraction = |denominator: f32| {
            dispatched(Fraction {
                numerator: boxed(1.0),
                denominator: boxed(denominator),
            })
        };
        let exponent = dispatched(Exponent {
            base: boxed(2.0),
            power: boxed(3.0),
        });
        let lookup = dispatched(Lookup {
            values: vec![10.0, 20.0],
            fallback: boxed(7.0),
        });
        let cases: [(&LevelBasedValue, i32, f32); 13] = [
            (&LevelBasedValue::Constant(2.5), 9, 2.5),
            (&linear, 3, 5.0),
            (&linear, 1, 1.0),
            (&squared, 3, 10.0),
            (&squared, 65_536, 1.0),
            (&clamped(5.0), 1, 2.0),
            (&clamped(-1.0), 1, 0.0),
            (&fraction(4.0), 1, 0.25),
            (&fraction(-0.0), 1, 0.0),
            (&exponent, 1, 8.0),
            (&lookup, 2, 20.0),
            (&lookup, 0, 7.0),
            (&lookup, 3, 7.0),
        ];
        for (value, level, expected) in cases {
            let calculated = value.calculate(level);
            assert_eq!(
                calculated.to_bits(),
                expected.to_bits(),
                "{value:?} at {level}: {calculated}"
            );
        }
        assert!(clamped(f32::NAN).calculate(1).is_nan(), "a NaN stays a NaN");
    }

    #[test]
    fn loot_num_021_clamped_with_swapped_bounds_gives_a_bound() {
        let swapped = dispatched(DispatchedLevelBasedValue::Clamped {
            value: boxed(5.0),
            min: 3.0,
            max: 1.0,
        });
        let calculated = swapped.calculate(1);
        assert!([1.0, 3.0].contains(&calculated), "{calculated}");
    }
}
