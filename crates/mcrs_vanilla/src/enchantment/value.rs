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
            LevelBasedValue::Clamped { value, min, max } => value.calculate(level).clamp(*min, *max),
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
#[serde(tag = "type", deny_unknown_fields)]
enum DispatchedLevelBasedValue {
    #[serde(rename = "minecraft:clamped")]
    Clamped {
        value: LevelBasedValue,
        min: f32,
        max: f32,
    },
    #[serde(rename = "minecraft:fraction")]
    Fraction {
        numerator: LevelBasedValue,
        denominator: LevelBasedValue,
    },
    #[serde(rename = "minecraft:levels_squared")]
    LevelsSquared { added: f32 },
    #[serde(rename = "minecraft:linear")]
    Linear {
        base: f32,
        per_level_above_first: f32,
    },
    #[serde(rename = "minecraft:exponent")]
    Exponent {
        base: LevelBasedValue,
        power: LevelBasedValue,
    },
    #[serde(rename = "minecraft:lookup")]
    Lookup {
        values: Vec<f32>,
        fallback: LevelBasedValue,
    },
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
                DispatchedLevelBasedValue::deserialize(de::value::MapAccessDeserializer::new(map))
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

/// Java's `FloatProviders.CODEC`, in the two forms the shipped enchantments use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatProvider {
    Constant(f32),
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
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
                let DispatchedFloatProvider::Uniform {
                    min_inclusive,
                    max_exclusive,
                } = DispatchedFloatProvider::deserialize(de::value::MapAccessDeserializer::new(
                    map,
                ))?;
                Ok(FloatProvider::Uniform {
                    min_inclusive,
                    max_exclusive,
                })
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
        }
    }
}

/// Java's loot `NumberProvider`, in the two forms the shipped enchantments use.
#[derive(Debug, Clone, PartialEq)]
pub enum NumberProvider {
    Constant(f32),
    EnchantmentLevel { amount: LevelBasedValue },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum DispatchedNumberProvider {
    #[serde(rename = "minecraft:enchantment_level")]
    EnchantmentLevel { amount: LevelBasedValue },
}

impl<'de> Deserialize<'de> for NumberProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = NumberProvider;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a number provider object")
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<NumberProvider, E> {
                Ok(NumberProvider::Constant(v as f32))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<NumberProvider, E> {
                Ok(NumberProvider::Constant(v as f32))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<NumberProvider, E> {
                Ok(NumberProvider::Constant(v as f32))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<NumberProvider, A::Error> {
                let DispatchedNumberProvider::EnchantmentLevel { amount } =
                    DispatchedNumberProvider::deserialize(de::value::MapAccessDeserializer::new(
                        map,
                    ))?;
                Ok(NumberProvider::EnchantmentLevel { amount })
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for NumberProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            NumberProvider::Constant(value) => s.serialize_f32(*value),
            NumberProvider::EnchantmentLevel { amount } => {
                DispatchedNumberProvider::EnchantmentLevel {
                    amount: amount.clone(),
                }
                .serialize(s)
            }
        }
    }
}

/// Java's `HolderSet` as a datapack writes it: a tag reference, one identifier,
/// or a list of identifiers. The three forms are kept apart so a re-encode is
/// the text the pack shipped.
#[derive(Debug, Clone, PartialEq)]
pub enum HolderSet {
    Tag(String),
    One(String),
    List(Vec<String>),
}

impl<'de> Deserialize<'de> for HolderSet {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = HolderSet;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a tag reference, an identifier, or a list of identifiers")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<HolderSet, E> {
                Ok(match v.strip_prefix('#') {
                    Some(tag) => HolderSet::Tag(tag.to_owned()),
                    None => HolderSet::One(v.to_owned()),
                })
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<HolderSet, A::Error> {
                let mut ids = Vec::with_capacity(seq.size_hint().unwrap_or(1));
                while let Some(id) = seq.next_element::<String>()? {
                    ids.push(id);
                }
                Ok(HolderSet::List(ids))
            }
        }

        d.deserialize_any(V)
    }
}

impl Serialize for HolderSet {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            HolderSet::Tag(tag) => s.serialize_str(&format!("#{tag}")),
            HolderSet::One(id) => s.serialize_str(id),
            HolderSet::List(ids) => ids.serialize(s),
        }
    }
}

/// Java's `MinMaxBounds`: a bare number is both ends, an object states either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bounds<T> {
    Exactly(T),
    Range { min: Option<T>, max: Option<T> },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BoundsRange<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<T>,
}

impl<'de, T> Deserialize<'de> for Bounds<T>
where
    T: Deserialize<'de> + Copy,
{
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for V<T>
        where
            T: Deserialize<'de> + Copy,
        {
            type Value = Bounds<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a number or a `min`/`max` object")
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Bounds<T>, E> {
                T::deserialize(de::value::F64Deserializer::new(v)).map(Bounds::Exactly)
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Bounds<T>, E> {
                T::deserialize(de::value::I64Deserializer::new(v)).map(Bounds::Exactly)
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Bounds<T>, E> {
                T::deserialize(de::value::U64Deserializer::new(v)).map(Bounds::Exactly)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Bounds<T>, A::Error> {
                let range =
                    BoundsRange::<T>::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(Bounds::Range {
                    min: range.min,
                    max: range.max,
                })
            }
        }

        d.deserialize_any(V(std::marker::PhantomData))
    }
}

impl<T: Serialize + Copy> Serialize for Bounds<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Bounds::Exactly(value) => value.serialize(s),
            Bounds::Range { min, max } => BoundsRange {
                min: *min,
                max: *max,
            }
            .serialize(s),
        }
    }
}
