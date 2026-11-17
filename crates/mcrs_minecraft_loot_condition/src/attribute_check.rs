use core::fmt;

use glam::DVec3;
use mcrs_minecraft_environment::environment_attribute_keys::EnvironmentAttribute;
use mcrs_minecraft_environment::{AttributeType, AttributeValue, default_value, info};
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};
use serde::de::{self, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::environment_view::EnvironmentView;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "AttributeCheckFile")]
pub(crate) struct AttributeCheck {
    pub(crate) attribute: EnvironmentAttribute,
    pub(crate) value: AttributeValue,
    matches_default: bool,
}

#[derive(Deserialize)]
struct AttributeCheckFile {
    attribute: EnvironmentAttribute,
    value: ValueFile,
}

struct ValueFile {
    shape: Shape,
    tag: NbtTag,
    number: Option<f64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Boolean,
    Number,
    String,
    Object,
    List,
}

impl TryFrom<AttributeCheckFile> for AttributeCheck {
    type Error = String;

    fn try_from(file: AttributeCheckFile) -> Result<Self, String> {
        let kind = info(file.attribute).kind;
        if !accepts(kind, file.value.shape) {
            return Err(format!("the value is not of the attribute type {kind:?}"));
        }
        let value = match (kind, file.value.number) {
            (AttributeType::Float | AttributeType::AngleDegrees, Some(number)) => {
                AttributeValue::read(kind, &NbtTag::Double(number))?
            }
            _ => AttributeValue::read(kind, &file.value.tag)?,
        };
        Ok(AttributeCheck {
            attribute: file.attribute,
            matches_default: same_value(&default_value(file.attribute), &value),
            value,
        })
    }
}

impl AttributeCheck {
    pub(crate) fn matches<W: EnvironmentView>(&self, world: &W, origin: Option<DVec3>) -> bool {
        let read = match info(self.attribute).positional {
            true => origin.and_then(|origin| world.environment_value(self.attribute, Some(origin))),
            false => world.environment_value(self.attribute, None),
        };
        match read {
            Some(value) => same_value(&value, &self.value),
            None => self.matches_default,
        }
    }
}

fn same_value(first: &AttributeValue, second: &AttributeValue) -> bool {
    use AttributeValue as V;
    match (first, second) {
        (V::Float(first), V::Float(second)) => first.to_bits() == second.to_bits(),
        (V::Rgb(first), V::Rgb(second)) => same_bits(first, second),
        (V::Argb(first), V::Argb(second)) => same_bits(first, second),
        (V::AmbientParticles(first), V::AmbientParticles(second)) => {
            first.len() == second.len()
                && first.iter().zip(second).all(|(first, second)| {
                    first.probability.to_bits() == second.probability.to_bits()
                        && first.particle == second.particle
                })
        }
        _ => first == second,
    }
}

fn same_bits(first: &[f32], second: &[f32]) -> bool {
    first
        .iter()
        .map(|value| value.to_bits())
        .eq(second.iter().map(|value| value.to_bits()))
}

fn accepts(kind: AttributeType, shape: Shape) -> bool {
    use AttributeType as T;
    match kind {
        T::Boolean => shape == Shape::Boolean,
        T::TriState => matches!(shape, Shape::Boolean | Shape::String),
        T::Float | T::AngleDegrees | T::Integer => shape == Shape::Number,
        T::RgbColor | T::ArgbColor => matches!(shape, Shape::String | Shape::Number | Shape::List),
        T::MoonPhase | T::Activity => shape == Shape::String,
        T::BedRule | T::Particle | T::BackgroundMusic | T::AmbientSounds | T::MobSpawnSettings => {
            shape == Shape::Object
        }
        T::AmbientParticles => shape == Shape::List,
    }
}

impl<'de> Deserialize<'de> for ValueFile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl ValueVisitor {
    fn value<'de, D: Deserializer<'de>>(shape: Shape, tag: D) -> Result<ValueFile, D::Error> {
        Ok(ValueFile {
            shape,
            tag: <NbtTag as Deserialize>::deserialize(tag)?,
            number: None,
        })
    }
}

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = ValueFile;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an environment attribute value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<ValueFile, E> {
        Self::value(Shape::Boolean, value.into_deserializer())
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<ValueFile, E> {
        Self::value(Shape::Number, value.into_deserializer())
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<ValueFile, E> {
        Self::value(Shape::Number, value.into_deserializer())
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<ValueFile, E> {
        let file = Self::value(Shape::Number, value.into_deserializer())?;
        Ok(ValueFile {
            number: Some(value),
            ..file
        })
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<ValueFile, E> {
        Self::value(Shape::String, value.into_deserializer())
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ValueFile, A::Error> {
        Self::value(Shape::Object, MapAccessDeserializer::new(map))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<ValueFile, A::Error> {
        Self::value(Shape::List, SeqAccessDeserializer::new(seq))
    }
}
