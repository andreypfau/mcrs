//! The `EnvironmentAttributes` registry: every attribute's type, default,
//! range and flags.
//!
//! Immutable data, built once. Nothing here holds an effective value — that is
//! composed from the layers on demand.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::int_value;
use mcrs_minecraft_protocol::item::Text;
use mcrs_minecraft_protocol::particle::ParticleOptions;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::modifier::Operation;
use super::value::{
    AmbientParticle, AmbientSounds, BackgroundMusic, BedRule, BedRuleCondition, MoonPhase, TriState,
};
use crate::attribute::MobSpawnSettings;

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    Bool(bool),
    TriState(TriState),
    Float(f32),
    /// Packed `0xAARRGGBB`.
    Color(u32),
    Integer(i32),
    MoonPhase(MoonPhase),
    // chisle: the name is not checked against the activity registry, which this crate does not depend on; lifts when it does
    Activity(ResourceLocation),
    BedRule(BedRule),
    Particle(Box<ParticleOptions>),
    AmbientParticles(Vec<AmbientParticle>),
    BackgroundMusic(Box<BackgroundMusic>),
    AmbientSounds(Box<AmbientSounds>),
    MobSpawns(Box<MobSpawnSettings>),
    /// `FloatWithAlpha`: the argument of a float `alpha_blend`.
    FloatWithAlpha {
        value: f32,
        alpha: f32,
    },
    /// `ColorModifier.BlendToGray`: the argument of a colour `blend_to_gray`.
    BlendToGray {
        brightness: f32,
        factor: f32,
    },
}

impl AttributeValue {
    fn is_of(&self, ty: AttributeType) -> bool {
        use AttributeType as T;
        use AttributeValue as V;

        matches!(
            (ty, self),
            (T::Boolean, V::Bool(_))
                | (T::TriState, V::TriState(_))
                | (T::Float | T::AngleDegrees, V::Float(_))
                | (T::RgbColor | T::ArgbColor, V::Color(_))
                | (T::Integer, V::Integer(_))
                | (T::MoonPhase, V::MoonPhase(_))
                | (T::Activity, V::Activity(_))
                | (T::BedRule, V::BedRule(_))
                | (T::Particle, V::Particle(_))
                | (T::AmbientParticles, V::AmbientParticles(_))
                | (T::BackgroundMusic, V::BackgroundMusic(_))
                | (T::AmbientSounds, V::AmbientSounds(_))
                | (T::MobSpawnSettings, V::MobSpawns(_))
        )
    }
}

/// `AttributeTypes`: what an attribute's value is and which modifiers apply to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributeType {
    Boolean,
    TriState,
    Float,
    AngleDegrees,
    RgbColor,
    ArgbColor,
    Integer,
    MoonPhase,
    Activity,
    BedRule,
    Particle,
    AmbientParticles,
    BackgroundMusic,
    AmbientSounds,
    MobSpawnSettings,
}

/// What a map found where an attribute entry belongs stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MapMeaning {
    /// The attribute's own value is an object whose every field is optional,
    /// so the game's value codec accepts any object first.
    Value,
    /// The attribute's own value is never an object, so this is the entry.
    Entry,
    /// Either; the first key tells.
    Peek,
}

impl AttributeType {
    /// Whether `op` is in this type's modifier library. `override` is universal.
    pub fn allows(self, op: Operation) -> bool {
        use AttributeType as T;
        use Operation::*;
        op == Override
            || matches!(
                (self, op),
                (T::Boolean, And | Nand | Or | Nor | Xor | Xnor)
                    | (
                        T::Float | T::AngleDegrees,
                        AlphaBlend | Add | Subtract | Multiply | Minimum | Maximum
                    )
                    | (
                        T::RgbColor | T::ArgbColor,
                        AlphaBlend | Add | Subtract | Multiply | BlendToGray
                    )
                    | (T::Integer, Add | Subtract | Multiply | Minimum | Maximum)
                    | (T::AmbientParticles, Append)
                    | (T::MobSpawnSettings, Overlay)
            )
    }

    /// Mirrors `Codec.either(attribute.valueCodec(), fullCodec)`: the value
    /// codec is tried first, so a value that happens to be an object is read as
    /// a value and never mistaken for an entry.
    pub(super) fn map_meaning(self) -> MapMeaning {
        use AttributeType as T;

        match self {
            T::Boolean
            | T::TriState
            | T::Float
            | T::AngleDegrees
            | T::RgbColor
            | T::ArgbColor
            | T::Integer
            | T::MoonPhase
            | T::Activity
            | T::AmbientParticles => MapMeaning::Entry,
            T::BedRule | T::Particle | T::MobSpawnSettings => MapMeaning::Peek,
            T::BackgroundMusic | T::AmbientSounds => MapMeaning::Value,
        }
    }
}

/// `AttributeRange`: the interval a value is validated against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttributeRange {
    Any,
    Bounded { min: f32, max: f32 },
}

impl AttributeRange {
    pub const UNIT: Self = Self::Bounded { min: 0.0, max: 1.0 };
    pub const UNIT_EPSILON: Self = Self::Bounded {
        min: 0.0,
        max: 0.9999999,
    };
    pub const NON_NEGATIVE: Self = Self::Bounded {
        min: 0.0,
        max: f32::INFINITY,
    };
}

/// One row of the registry: everything that is fixed about an attribute.
#[derive(Debug, Clone)]
pub struct AttributeSpec {
    pub id: &'static str,
    pub ty: AttributeType,
    pub default: AttributeValue,
    pub range: AttributeRange,
    pub syncable: bool,
    pub positional: bool,
    pub spatially_interpolated: bool,
}

impl AttributeSpec {
    pub fn value_seed(&self) -> ArgumentSeed<'_> {
        self.argument_seed(Operation::Override)
    }

    pub fn argument_seed(&self, op: Operation) -> ArgumentSeed<'_> {
        ArgumentSeed { spec: self, op }
    }

    fn check_range(&self, value: AttributeValue) -> Result<AttributeValue, AttributeError> {
        if let (AttributeRange::Bounded { min, max }, AttributeValue::Float(v)) =
            (self.range, &value)
            && (*v < min || *v > max)
        {
            return Err(malformed(
                self.id,
                format!("{v} is not in range [{min}; {max}]"),
            ));
        }
        Ok(value)
    }

    /// `AttributeRange.sanitize`: clamp a composed value into the attribute's
    /// range. Layers compose freely and only the result has to be legal.
    pub fn sanitize(&self, value: AttributeValue) -> AttributeValue {
        match (self.range, &value) {
            (AttributeRange::Bounded { min, max }, AttributeValue::Float(v)) => {
                AttributeValue::Float(v.clamp(min, max))
            }
            _ => value,
        }
    }

    /// The codec the argument of `op` is read and written with.
    ///
    /// Only `override` takes the attribute's own value; every other operation
    /// carries the argument its modifier declares, which is why the choice is
    /// dispatched on the (attribute type, operation) pair.
    fn argument_shape(&self, op: Operation) -> Result<ArgumentShape, AttributeError> {
        use AttributeType as T;
        use Operation::*;

        if !self.ty.allows(op) {
            return Err(malformed(
                self.id,
                format!("{op:?} is not a valid modifier for {:?}", self.ty),
            ));
        }
        Ok(match (self.ty, op) {
            (_, Override) => ArgumentShape::Value,
            // FloatModifier.Simple takes a plain float, unconstrained by the
            // attribute's own range.
            (T::Float | T::AngleDegrees, Add | Subtract | Multiply | Minimum | Maximum) => {
                ArgumentShape::Typed(T::Float)
            }
            (T::Integer, Add | Subtract | Multiply | Minimum | Maximum) => {
                ArgumentShape::Typed(T::Integer)
            }
            (T::Boolean, And | Nand | Or | Nor | Xor | Xnor) => ArgumentShape::Typed(T::Boolean),
            // `ColorModifier.ADD`/`SUBTRACT` are one instance shared by both
            // colour libraries, so their argument shape cannot vary by attribute
            // type; `multiply` is split into MULTIPLY_RGB and MULTIPLY_ARGB
            // because only `ARGB.multiply` consumes the argument's alpha.
            (T::RgbColor | T::ArgbColor, Add | Subtract) | (T::RgbColor, Multiply) => {
                ArgumentShape::Typed(T::RgbColor)
            }
            (T::RgbColor | T::ArgbColor, AlphaBlend) | (T::ArgbColor, Multiply) => {
                ArgumentShape::Typed(T::ArgbColor)
            }
            (T::Float | T::AngleDegrees, AlphaBlend) => ArgumentShape::FloatWithAlpha,
            (T::RgbColor | T::ArgbColor, BlendToGray) => ArgumentShape::BlendToGray,
            (T::AmbientParticles, Append) => ArgumentShape::Typed(T::AmbientParticles),
            (T::MobSpawnSettings, Overlay) => ArgumentShape::Typed(T::MobSpawnSettings),
            _ => return Err(malformed(self.id, format!("no argument codec for {op:?}"))),
        })
    }

    /// Write `value` back in the form the argument codec of `op` encodes with.
    ///
    /// Vanilla's codecs are symmetric, so this is what a re-serialized timeline
    /// or attribute entry has to produce: the one form its codec writes.
    pub fn serialize_argument<S: Serializer>(
        &self,
        op: Operation,
        value: &AttributeValue,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;

        let shape = self.argument_shape(op).map_err(S::Error::custom)?;
        let ty = match (shape, value) {
            (ArgumentShape::Value, _) => self.ty,
            (ArgumentShape::Typed(ty), _) => ty,
            (ArgumentShape::FloatWithAlpha, AttributeValue::FloatWithAlpha { value, alpha }) => {
                return if *alpha == 1.0 {
                    serializer.serialize_f32(*value)
                } else {
                    FloatWithAlphaFields {
                        value: *value,
                        alpha: *alpha,
                    }
                    .serialize(serializer)
                };
            }
            (ArgumentShape::BlendToGray, AttributeValue::BlendToGray { brightness, factor }) => {
                return BlendToGrayFields {
                    brightness: *brightness,
                    factor: *factor,
                }
                .serialize(serializer);
            }
            (shape, value) => {
                return Err(S::Error::custom(malformed(
                    self.id,
                    format!("{value:?} is not a {shape:?} argument"),
                )));
            }
        };
        serialize_typed(self.id, ty, value, serializer)
    }
}

pub struct ArgumentRef<'a> {
    pub spec: &'a AttributeSpec,
    pub op: Operation,
    pub value: &'a AttributeValue,
}

impl Serialize for ArgumentRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.spec
            .serialize_argument(self.op, self.value, serializer)
    }
}

#[derive(Debug, Clone, Copy)]
enum ArgumentShape {
    /// The attribute's own value codec, validated against its range.
    Value,
    Typed(AttributeType),
    FloatWithAlpha,
    BlendToGray,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FloatWithAlphaFields {
    value: f32,
    #[serde(default = "opaque", deserialize_with = "unit_f32")]
    alpha: f32,
}

fn opaque() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlendToGrayFields {
    #[serde(deserialize_with = "unit_f32")]
    brightness: f32,
    #[serde(deserialize_with = "unit_f32")]
    factor: f32,
}

pub(crate) fn unit_f32<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    let value = f32::deserialize(deserializer)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(de::Error::custom(format_args!(
            "{value} is not in range [0; 1]"
        )));
    }
    Ok(value)
}

/// An argument as far as the attribute's type alone determines it.
///
/// Vanilla decodes from an already-materialized `DynamicOps` tree, so the field
/// that selects an argument codec may follow the value it selects for — a
/// track's `modifier` follows its `keyframes`. Serde streams, so what the type
/// leaves open (a bare float is a float or an `alpha_blend` with alpha 1; a
/// six-digit colour is an rgb colour and an eight-digit one an argb colour) is
/// kept as such until the modifier has been read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Draft {
    Value(AttributeValue),
    Number(f32),
    Color { packed: u32, form: ColorForm },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorForm {
    Rgb,
    Argb,
    /// A packed integer, which both codecs take.
    Unspecified,
}

impl Draft {
    pub(crate) fn finish(
        self,
        spec: &AttributeSpec,
        op: Operation,
    ) -> Result<AttributeValue, AttributeError> {
        let shape = spec.argument_shape(op)?;
        let wrong = || {
            malformed(
                spec.id,
                format!("{op:?} takes a {shape:?} argument, which this is not"),
            )
        };
        let typed = |draft: Draft, ty: AttributeType| match (ty, draft) {
            (AttributeType::Float | AttributeType::AngleDegrees, Draft::Number(v)) => {
                Some(AttributeValue::Float(v))
            }
            (
                AttributeType::RgbColor,
                Draft::Color {
                    packed,
                    form: ColorForm::Rgb | ColorForm::Unspecified,
                },
            )
            | (
                AttributeType::ArgbColor,
                Draft::Color {
                    packed,
                    form: ColorForm::Argb | ColorForm::Unspecified,
                },
            ) => Some(AttributeValue::Color(packed)),
            (ty, Draft::Value(value)) if value.is_of(ty) => Some(value),
            _ => None,
        };
        match (shape, self) {
            (ArgumentShape::FloatWithAlpha, Draft::Number(value)) => {
                Ok(AttributeValue::FloatWithAlpha { value, alpha: 1.0 })
            }
            (
                ArgumentShape::FloatWithAlpha,
                Draft::Value(value @ AttributeValue::FloatWithAlpha { .. }),
            )
            | (
                ArgumentShape::BlendToGray,
                Draft::Value(value @ AttributeValue::BlendToGray { .. }),
            ) => Ok(value),
            (ArgumentShape::Value, draft) => {
                spec.check_range(typed(draft, spec.ty).ok_or_else(wrong)?)
            }
            (ArgumentShape::Typed(ty), draft) => typed(draft, ty).ok_or_else(wrong),
            _ => Err(wrong()),
        }
    }
}

pub struct ArgumentSeed<'a> {
    spec: &'a AttributeSpec,
    op: Operation,
}

impl<'de> DeserializeSeed<'de> for ArgumentSeed<'_> {
    type Value = AttributeValue;

    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<AttributeValue, D::Error> {
        DraftSeed(self.spec)
            .deserialize(deserializer)?
            .finish(self.spec, self.op)
            .map_err(de::Error::custom)
    }
}

pub(crate) struct DraftSeed<'a>(pub(crate) &'a AttributeSpec);

impl<'de> DeserializeSeed<'de> for DraftSeed<'_> {
    type Value = Draft;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Draft, D::Error> {
        read_draft(self.0.ty, deserializer).map_err(|error| {
            de::Error::custom(malformed(
                self.0.id,
                format!("{error}; this is not a valid {:?} value", self.0.ty),
            ))
        })
    }
}

fn read_draft<'de, D: Deserializer<'de>>(ty: AttributeType, d: D) -> Result<Draft, D::Error> {
    use AttributeType as T;
    use AttributeValue as V;

    Ok(Draft::Value(match ty {
        T::Boolean => V::Bool(bool::deserialize(d)?),
        T::TriState => V::TriState(TriState::deserialize(d)?),
        T::Float | T::AngleDegrees => return d.deserialize_any(FloatDraft),
        T::Integer => V::Integer(int_value(d)?),
        T::RgbColor | T::ArgbColor => return d.deserialize_any(ColorDraft),
        T::MoonPhase => V::MoonPhase(MoonPhase::deserialize(d)?),
        T::Activity => V::Activity(ResourceLocation::deserialize(d)?),
        T::BedRule => V::BedRule(BedRule::deserialize(d)?),
        T::Particle => V::Particle(Box::new(ParticleOptions::deserialize(d)?)),
        T::AmbientParticles => V::AmbientParticles(Vec::deserialize(d)?),
        T::BackgroundMusic => V::BackgroundMusic(Box::new(BackgroundMusic::deserialize(d)?)),
        T::AmbientSounds => V::AmbientSounds(Box::new(AmbientSounds::deserialize(d)?)),
        T::MobSpawnSettings => V::MobSpawns(Box::new(MobSpawnSettings::deserialize(d)?)),
    }))
}

/// A bare float implies `alpha: 1`.
struct FloatDraft;

impl<'de> Visitor<'de> for FloatDraft {
    type Value = Draft;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a float or a `{value, alpha}` object")
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Draft, E> {
        Ok(Draft::Number(v as f32))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Draft, E> {
        Ok(Draft::Number(v as f32))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Draft, E> {
        Ok(Draft::Number(v as f32))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Draft, A::Error> {
        let FloatWithAlphaFields { value, alpha } =
            FloatWithAlphaFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
        Ok(Draft::Value(AttributeValue::FloatWithAlpha {
            value,
            alpha,
        }))
    }
}

/// A `#`-prefixed hex string of six or eight digits, a packed integer, or the
/// float vector form — three components for rgb, four for argb with the alpha
/// last — and, for the colour modifiers, the blend-to-gray object.
struct ColorDraft;

impl<'de> Visitor<'de> for ColorDraft {
    type Value = Draft;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a colour, as a hex string, an integer or 3 or 4 floats")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Draft, E> {
        let invalid = || E::invalid_value(de::Unexpected::Str(v), &"`#` and 6 or 8 hex digits");
        let hex = v.strip_prefix('#').ok_or_else(invalid)?;
        let (form, opaque) = match hex.len() {
            6 => (ColorForm::Rgb, 0xFF00_0000),
            8 => (ColorForm::Argb, 0),
            _ => return Err(invalid()),
        };
        let raw = u32::from_str_radix(hex, 16).map_err(|_| invalid())?;
        Ok(Draft::Color {
            packed: raw | opaque,
            form,
        })
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Draft, E> {
        Ok(Draft::Color {
            packed: v as u32,
            form: ColorForm::Unspecified,
        })
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Draft, E> {
        Ok(Draft::Color {
            packed: v as u32,
            form: ColorForm::Unspecified,
        })
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Draft, A::Error> {
        let mut components = [0.0f32; 4];
        let mut len = 0;
        while let Some(component) = seq.next_element::<f32>()? {
            *components
                .get_mut(len)
                .ok_or_else(|| de::Error::invalid_length(len + 1, &"3 or 4 colour components"))? =
                component;
            len += 1;
        }
        let (form, alpha) = match len {
            3 => (ColorForm::Rgb, 255),
            4 => (ColorForm::Argb, as_8bit_channel(components[3])),
            _ => return Err(de::Error::invalid_length(len, &"3 or 4 colour components")),
        };
        Ok(Draft::Color {
            packed: alpha << 24
                | as_8bit_channel(components[0]) << 16
                | as_8bit_channel(components[1]) << 8
                | as_8bit_channel(components[2]),
            form,
        })
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Draft, A::Error> {
        let BlendToGrayFields { brightness, factor } =
            BlendToGrayFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
        Ok(Draft::Value(AttributeValue::BlendToGray {
            brightness,
            factor,
        }))
    }
}

fn as_8bit_channel(value: f32) -> u32 {
    (value * 255.0).floor() as i32 as u32 & 0xFF
}

fn serialize_typed<S: Serializer>(
    id: &'static str,
    ty: AttributeType,
    value: &AttributeValue,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use AttributeType as T;
    use AttributeValue as V;
    use serde::ser::Error;

    let wrong = || S::Error::custom(malformed(id, format!("{value:?} is not a {ty:?} value")));
    match (ty, value) {
        (T::Boolean, V::Bool(v)) => serializer.serialize_bool(*v),
        (T::TriState, V::TriState(v)) => v.serialize(serializer),
        (T::Float | T::AngleDegrees, V::Float(v)) => serializer.serialize_f32(*v),
        (T::Integer, V::Integer(v)) => serializer.serialize_i32(*v),
        (T::RgbColor, V::Color(packed)) => {
            serializer.serialize_str(&format!("#{:06x}", packed & 0x00FF_FFFF))
        }
        (T::ArgbColor, V::Color(packed)) => serializer.serialize_str(&format!("#{packed:08x}")),
        (T::MoonPhase, V::MoonPhase(v)) => v.serialize(serializer),
        (T::Activity, V::Activity(v)) => v.serialize(serializer),
        (T::BedRule, V::BedRule(v)) => v.serialize(serializer),
        (T::Particle, V::Particle(v)) => v.serialize(serializer),
        (T::AmbientParticles, V::AmbientParticles(v)) => v.serialize(serializer),
        (T::BackgroundMusic, V::BackgroundMusic(v)) => v.serialize(serializer),
        (T::AmbientSounds, V::AmbientSounds(v)) => v.serialize(serializer),
        (T::MobSpawnSettings, V::MobSpawns(v)) => v.serialize(serializer),
        _ => Err(wrong()),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AttributeError {
    #[error("unknown environment attribute `{0}`; the registry is behind the game version")]
    UnknownAttribute(String),
    #[error("environment attribute `{id}`: {reason}")]
    Malformed { id: &'static str, reason: String },
}

pub(super) fn malformed(id: &'static str, reason: impl Into<String>) -> AttributeError {
    AttributeError::Malformed {
        id,
        reason: reason.into(),
    }
}

/// A static rather than a Bevy resource: every entry point into this table is a
/// `serde` impl, and `Deserialize` has no way to reach a `World`.
pub static ENVIRONMENT_ATTRIBUTES: LazyLock<BTreeMap<&'static str, AttributeSpec>> =
    LazyLock::new(|| table().into_iter().map(|spec| (spec.id, spec)).collect());

/// The spec for `id`, or `None` if the game registers no such attribute.
pub fn attribute(id: &str) -> Option<&'static AttributeSpec> {
    ENVIRONMENT_ATTRIBUTES.get(id)
}

/// Whether the client is allowed to see this attribute.
pub fn is_syncable(id: &str) -> bool {
    attribute(id).is_some_and(|spec| spec.syncable)
}

// ── The table ────────────────────────────────────────────────────────────────

const SYNC: u8 = 1;
const INTERP: u8 = 2;
const NOT_POSITIONAL: u8 = 4;

fn row(
    id: &'static str,
    ty: AttributeType,
    default: AttributeValue,
    range: AttributeRange,
    flags: u8,
) -> AttributeSpec {
    AttributeSpec {
        id,
        ty,
        default,
        range,
        syncable: flags & SYNC != 0,
        positional: flags & NOT_POSITIONAL == 0,
        spatially_interpolated: flags & INTERP != 0,
    }
}

#[rustfmt::skip]
fn table() -> Vec<AttributeSpec> {
    use AttributeRange as R;
    use AttributeType as T;
    use AttributeValue as V;

    let color = |packed: i32| V::Color(packed as u32);
    let float = V::Float;
    let flag = V::Bool;
    let activity = || V::Activity(ResourceLocation::minecraft("idle"));
    let bed_rule = |can_set_spawn, destroy_on_leave| V::BedRule(BedRule {
        can_sleep: BedRuleCondition::WhenDark,
        can_set_spawn,
        destroy_on_use: false,
        destroy_on_leave,
        error_message: Some(Text::translate("block.minecraft.bed.no_sleep", Vec::new())),
    });

    vec![
        row("minecraft:visual/fog_color", T::RgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/fog_start_distance", T::Float, float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/fog_end_distance", T::Float, float(1024.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/sky_fog_end_distance", T::Float, float(512.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/cloud_fog_end_distance", T::Float, float(2048.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/water_fog_color", T::RgbColor, color(-16448205), R::Any, SYNC | INTERP),
        row("minecraft:visual/water_fog_start_distance", T::Float, float(-8.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/water_fog_end_distance", T::Float, float(96.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/sky_color", T::RgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/sunrise_sunset_color", T::ArgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/cloud_color", T::ArgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/cloud_height", T::Float, float(192.33), R::Any, SYNC | INTERP),
        row("minecraft:visual/sun_angle", T::AngleDegrees, float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/moon_angle", T::AngleDegrees, float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/star_angle", T::AngleDegrees, float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/moon_phase", T::MoonPhase, V::MoonPhase(MoonPhase::FullMoon), R::Any, SYNC),
        row("minecraft:visual/star_brightness", T::Float, float(0.0), R::UNIT, SYNC | INTERP),
        row("minecraft:visual/has_sky_occluder", T::Boolean, flag(false), R::Any, SYNC),
        row("minecraft:visual/block_light_tint", T::RgbColor, color(-10100), R::Any, SYNC | INTERP),
        row("minecraft:visual/sky_light_color", T::RgbColor, color(-1), R::Any, SYNC | INTERP),
        row("minecraft:visual/sky_light_factor", T::Float, float(1.0), R::UNIT, SYNC | INTERP),
        row("minecraft:visual/night_vision_color", T::RgbColor, color(-6710887), R::Any, SYNC | INTERP),
        row("minecraft:visual/ambient_light_color", T::RgbColor, color(-16777216), R::Any, SYNC | INTERP),
        row("minecraft:visual/default_dripstone_particle", T::Particle, V::Particle(Box::new(ParticleOptions::DrippingDripstoneWater)), R::Any, SYNC),
        row("minecraft:visual/ambient_particles", T::AmbientParticles, V::AmbientParticles(Vec::new()), R::Any, SYNC),
        row("minecraft:audio/background_music", T::BackgroundMusic, V::BackgroundMusic(Box::default()), R::Any, SYNC),
        row("minecraft:audio/music_volume", T::Float, float(1.0), R::UNIT, SYNC),
        row("minecraft:audio/ambient_sounds", T::AmbientSounds, V::AmbientSounds(Box::default()), R::Any, SYNC),
        row("minecraft:audio/firefly_bush_sounds", T::Boolean, flag(false), R::Any, SYNC),
        row("minecraft:gameplay/sky_light_level", T::Float, float(15.0), R::Bounded { min: 0.0, max: 15.0 }, SYNC | NOT_POSITIONAL),
        row("minecraft:gameplay/can_start_raid", T::Boolean, flag(true), R::Any, 0),
        row("minecraft:gameplay/water_evaporates", T::Boolean, flag(false), R::Any, SYNC),
        row("minecraft:gameplay/bed_rule", T::BedRule, bed_rule(BedRuleCondition::Always, false), R::Any, 0),
        row("minecraft:gameplay/straw_bed_rule", T::BedRule, bed_rule(BedRuleCondition::Never, true), R::Any, 0),
        row("minecraft:gameplay/respawn_anchor_works", T::Boolean, flag(false), R::Any, 0),
        row("minecraft:gameplay/nether_portal_spawns_piglin", T::Boolean, flag(false), R::Any, 0),
        row("minecraft:gameplay/fast_lava", T::Boolean, flag(false), R::Any, SYNC | NOT_POSITIONAL),
        row("minecraft:gameplay/increased_fire_burnout", T::Boolean, flag(false), R::Any, 0),
        row("minecraft:gameplay/eyeblossom_open", T::TriState, V::TriState(TriState::Default), R::Any, 0),
        row("minecraft:gameplay/turtle_egg_hatch_chance", T::Float, float(0.002), R::UNIT, 0),
        row("minecraft:gameplay/piglins_zombify", T::Boolean, flag(true), R::Any, SYNC),
        row("minecraft:gameplay/snow_golem_melts", T::Boolean, flag(false), R::Any, 0),
        row("minecraft:gameplay/creaking_active", T::Boolean, flag(false), R::Any, SYNC),
        row("minecraft:gameplay/surface_slime_spawn_chance", T::Float, float(0.0), R::UNIT, 0),
        row("minecraft:gameplay/cat_waking_up_gift_chance", T::Float, float(0.0), R::UNIT, 0),
        row("minecraft:gameplay/bees_stay_in_hive", T::Boolean, flag(false), R::Any, 0),
        row("minecraft:gameplay/monsters_burn", T::Boolean, flag(false), R::Any, 0),
        row("minecraft:gameplay/can_pillager_patrol_spawn", T::Boolean, flag(true), R::Any, 0),
        row("minecraft:gameplay/natural_mob_spawns", T::MobSpawnSettings, V::MobSpawns(Box::default()), R::Any, 0),
        row("minecraft:gameplay/creature_world_gen_spawn_probability", T::Float, float(0.1), R::UNIT_EPSILON, 0),
        row("minecraft:gameplay/villager_activity", T::Activity, activity(), R::Any, 0),
        row("minecraft:gameplay/baby_villager_activity", T::Activity, activity(), R::Any, 0),
    ]
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_registry::static_report::from_report;
    use serde_json::json;

    use super::*;

    fn value(
        spec: &AttributeSpec,
        json: serde_json::Value,
    ) -> Result<AttributeValue, serde_json::Error> {
        spec.value_seed().deserialize(json)
    }

    fn argument(
        spec: &AttributeSpec,
        op: Operation,
        json: serde_json::Value,
    ) -> Result<AttributeValue, serde_json::Error> {
        spec.argument_seed(op).deserialize(json)
    }

    #[test]
    fn registry_holds_every_attribute() {
        assert_eq!(
            table().len(),
            ENVIRONMENT_ATTRIBUTES.len(),
            "ids must be unique"
        );

        let syncable: Vec<_> = ENVIRONMENT_ATTRIBUTES
            .values()
            .filter(|spec| spec.syncable)
            .collect();
        assert_eq!(syncable.len(), 34);
        assert!(
            syncable
                .iter()
                .filter(|spec| spec.id.starts_with("minecraft:gameplay/"))
                .count()
                == 5
        );

        for spec in ENVIRONMENT_ATTRIBUTES.values() {
            assert!(
                spec.id.starts_with("minecraft:"),
                "{} is not namespaced",
                spec.id
            );
        }
    }

    #[test]
    fn the_table_is_the_registry_in_names_and_order() {
        let report = from_report(
            &std::fs::read(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets/mcrs/reports/registries.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let registry: Vec<String> = report
            .table("minecraft:environment_attribute")
            .unwrap()
            .names()
            .iter()
            .map(ToString::to_string)
            .collect();
        let ours: Vec<String> = table().iter().map(|spec| spec.id.to_owned()).collect();
        assert_eq!(ours, registry);
    }

    #[test]
    fn every_default_is_a_value_of_its_type() {
        for spec in ENVIRONMENT_ATTRIBUTES.values() {
            assert!(spec.default.is_of(spec.ty), "{}", spec.id);
        }
    }

    #[test]
    fn an_attribute_value_stays_small_enough_to_clone_per_frame() {
        assert!(size_of::<AttributeValue>() <= 32);
    }

    #[test]
    fn values_and_arguments_parse_by_the_attribute_type() {
        let sky_color = attribute("minecraft:visual/sky_color").unwrap();
        assert_eq!(
            value(sky_color, json!("#78a7ff")).unwrap(),
            AttributeValue::Color(0xFF78_A7FF)
        );
        assert!(
            value(sky_color, json!("#ccffffff")).is_err(),
            "rgb takes 6 digits"
        );

        let cloud_color = attribute("minecraft:visual/cloud_color").unwrap();
        assert_eq!(
            value(cloud_color, json!("#ccffffff")).unwrap(),
            AttributeValue::Color(0xCCFF_FFFF)
        );

        let volume = attribute("minecraft:audio/music_volume").unwrap();
        assert_eq!(
            value(volume, json!(0.5)).unwrap(),
            AttributeValue::Float(0.5)
        );
        assert!(value(volume, json!(1.5)).is_err(), "music_volume is UNIT");

        assert_eq!(
            value(sky_color, json!([1.0, 0.5, 0.0])).unwrap(),
            AttributeValue::Color(0xFFFF_7F00)
        );
        // the fourth component is the alpha
        assert_eq!(
            value(cloud_color, json!([1.0, 0.5, 0.0, 0.5])).unwrap(),
            AttributeValue::Color(0x7FFF_7F00)
        );
        assert!(
            value(sky_color, json!([1.0, 0.5, 0.0, 0.5])).is_err(),
            "rgb takes 3"
        );
        assert!(
            value(cloud_color, json!([1.0, 0.5, 0.0])).is_err(),
            "argb takes 4"
        );

        for spec in [sky_color, cloud_color] {
            assert_eq!(
                argument(spec, Operation::Add, json!("#102030")).unwrap(),
                AttributeValue::Color(0xFF10_2030),
                "{} takes the six-digit form for add",
                spec.id
            );
            assert!(
                argument(spec, Operation::Subtract, json!("#80102030")).is_err(),
                "{} must reject an eight-digit subtract argument",
                spec.id
            );
        }
        assert_eq!(
            argument(cloud_color, Operation::Multiply, json!("#80102030")).unwrap(),
            AttributeValue::Color(0x8010_2030)
        );
        assert!(argument(sky_color, Operation::Multiply, json!("#80102030")).is_err());
    }

    #[test]
    fn multiply_argument_escapes_the_attribute_range() {
        // FloatModifier.Simple validates the argument as a plain float, so 0.85
        // is legal here even though the attribute itself is NON_NEGATIVE.
        let end = attribute("minecraft:visual/water_fog_end_distance").unwrap();
        assert_eq!(
            argument(end, Operation::Multiply, json!(0.85)).unwrap(),
            AttributeValue::Float(0.85)
        );
        assert!(argument(end, Operation::Or, json!(true)).is_err());
    }

    #[test]
    fn an_argument_its_modifier_does_not_take_fails() {
        let sky_color = attribute("minecraft:visual/sky_color").unwrap();
        let error = argument(
            sky_color,
            Operation::Add,
            json!({"brightness": 0.5, "factor": 0.5}),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("minecraft:visual/sky_color"), "{error}");
        assert!(error.contains("Add"), "{error}");

        let error = argument(sky_color, Operation::BlendToGray, json!("#102030"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("BlendToGray"), "{error}");

        let volume = attribute("minecraft:audio/music_volume").unwrap();
        let error = argument(volume, Operation::Add, json!({"value": 0.5, "alpha": 0.5}))
            .unwrap_err()
            .to_string();
        assert!(error.contains("minecraft:audio/music_volume"), "{error}");
    }
}
