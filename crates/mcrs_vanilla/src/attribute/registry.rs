//! The `EnvironmentAttributes` registry: every attribute's type, default,
//! range and flags.
//!
//! Immutable data, built once. Nothing here holds an effective value — that is
//! composed from the layers on demand.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;
use serde_json::{Value, json};

use super::modifier::Operation;
use crate::biome::MobSpawnSettings;

/// An attribute value, parsed according to its [`AttributeType`].
///
/// The types the server never inspects keep their payload as raw JSON in
/// [`AttributeValue::Opaque`].
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    Bool(bool),
    Float(f32),
    /// Packed `0xAARRGGBB`.
    Color(u32),
    Integer(i32),
    List(Vec<Value>),
    MobSpawns(Box<MobSpawnSettings>),
    /// `FloatWithAlpha`: the argument of a float `alpha_blend`.
    FloatWithAlpha { value: f32, alpha: f32 },
    /// `ColorModifier.BlendToGray`: the argument of a colour `blend_to_gray`.
    BlendToGray { brightness: f32, factor: f32 },
    Opaque(Value),
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

    /// Whether `value` is this type's own value shape rather than the
    /// `{argument, modifier}` entry shape.
    ///
    /// Mirrors `Codec.either(attribute.valueCodec(), fullCodec)`: the value
    /// codec is tried first, so a value that happens to be an object is read as
    /// a value and never mistaken for an entry.
    pub fn matches_value(self, value: &Value) -> bool {
        match self {
            Self::Boolean => value.is_boolean(),
            Self::Float | Self::AngleDegrees => value.is_number(),
            Self::Integer => value.is_i64(),
            // a hex string, a packed int, or an rgb(a) vector — never an object
            Self::RgbColor | Self::ArgbColor => !value.is_object(),
            Self::AmbientParticles => value.is_array(),
            // Every field is optional, so without `deny_unknown_fields` this
            // would also accept `{argument, modifier}` and swallow every overlay.
            Self::MobSpawnSettings => MobSpawnSettings::deserialize(value).is_ok(),
            // The remaining types have an empty modifier library, and
            // `Entry.createCodec` encodes `override` through `Either.left`, so
            // `{argument, modifier: override}` is a shape nothing can emit.
            // Anything here is the value, kept verbatim rather than validated.
            _ => true,
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
    pub const UNIT_EPSILON: Self = Self::Bounded { min: 0.0, max: 0.9999999 };
    pub const NON_NEGATIVE: Self = Self::Bounded { min: 0.0, max: f32::INFINITY };
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
    /// Parse `value` as this attribute's own value, validating it against the range.
    pub fn parse_value(&self, value: &Value) -> Result<AttributeValue, AttributeError> {
        let parsed = parse_typed(self.id, self.ty, value)?;
        if let (AttributeRange::Bounded { min, max }, AttributeValue::Float(v)) =
            (self.range, &parsed)
            && (*v < min || *v > max)
        {
            return Err(malformed(self.id, format!("{v} is not in range [{min}; {max}]")));
        }
        Ok(parsed)
    }

    /// Parse the argument of `op` applied to this attribute.
    ///
    /// Only `override` takes the attribute's own value; every other operation
    /// carries the argument its modifier declares, which is why the parse is
    /// dispatched on the (attribute type, operation) pair.
    pub fn parse_argument(
        &self,
        op: Operation,
        value: &Value,
    ) -> Result<AttributeValue, AttributeError> {
        use AttributeType as T;
        use Operation::*;

        if !self.ty.allows(op) {
            return Err(malformed(
                self.id,
                format!("{op:?} is not a valid modifier for {:?}", self.ty),
            ));
        }
        match (self.ty, op) {
            (_, Override) => self.parse_value(value),
            // FloatModifier.Simple takes a plain float, unconstrained by the
            // attribute's own range.
            (T::Float | T::AngleDegrees, Add | Subtract | Multiply | Minimum | Maximum) => {
                parse_typed(self.id, T::Float, value)
            }
            (T::Integer, Add | Subtract | Multiply | Minimum | Maximum) => {
                parse_typed(self.id, T::Integer, value)
            }
            (T::Boolean, And | Nand | Or | Nor | Xor | Xnor) => {
                parse_typed(self.id, T::Boolean, value)
            }
            // `ColorModifier.ADD`/`SUBTRACT` are one instance shared by both
            // colour libraries, so their argument shape cannot vary by attribute
            // type; `multiply` is split into MULTIPLY_RGB and MULTIPLY_ARGB
            // because only `ARGB.multiply` consumes the argument's alpha.
            (T::RgbColor | T::ArgbColor, Add | Subtract) | (T::RgbColor, Multiply) => {
                parse_typed(self.id, T::RgbColor, value)
            }
            (T::ArgbColor, Multiply) | (T::RgbColor | T::ArgbColor, AlphaBlend) => {
                parse_typed(self.id, T::ArgbColor, value)
            }
            (T::Float | T::AngleDegrees, AlphaBlend) => parse_float_with_alpha(self.id, value),
            (T::RgbColor | T::ArgbColor, BlendToGray) => parse_blend_to_gray(self.id, value),
            (T::AmbientParticles, Append) => parse_typed(self.id, T::AmbientParticles, value),
            (T::MobSpawnSettings, Overlay) => parse_typed(self.id, T::MobSpawnSettings, value),
            _ => Err(malformed(self.id, format!("no argument codec for {op:?}"))),
        }
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
    AttributeError::Malformed { id, reason: reason.into() }
}

/// A static rather than a Bevy resource: every entry point into this table is a
/// `serde` impl, and `Deserialize` has no way to reach a `World`.
pub static ENVIRONMENT_ATTRIBUTES: LazyLock<BTreeMap<&'static str, AttributeSpec>> =
    LazyLock::new(|| table().into_iter().map(|spec| (spec.id, spec)).collect());

/// The spec for `id`, or `None` if no such attribute exists in 26.3.
pub fn attribute(id: &str) -> Option<&'static AttributeSpec> {
    ENVIRONMENT_ATTRIBUTES.get(id)
}

/// Whether the client is allowed to see this attribute.
pub fn is_syncable(id: &str) -> bool {
    attribute(id).is_some_and(|spec| spec.syncable)
}

/// `FloatWithAlpha.CODEC`: a bare float, which implies `alpha: 1`, or the full
/// `{value, alpha}` form.
fn parse_float_with_alpha(id: &'static str, value: &Value) -> Result<AttributeValue, AttributeError> {
    let wrong = || malformed(id, format!("{value} is not a valid alpha_blend argument"));
    if let Some(v) = value.as_f64() {
        return Ok(AttributeValue::FloatWithAlpha { value: v as f32, alpha: 1.0 });
    }
    let fields = value.as_object().ok_or_else(wrong)?;
    let alpha = match fields.get("alpha") {
        Some(alpha) => unit(id, "alpha", alpha.as_f64().ok_or_else(wrong)? as f32)?,
        None => 1.0,
    };
    let value = fields.get("value").and_then(Value::as_f64).ok_or_else(wrong)? as f32;
    Ok(AttributeValue::FloatWithAlpha { value, alpha })
}

/// `ColorModifier.BlendToGray.CODEC`: both fields required and in `[0; 1]`.
fn parse_blend_to_gray(id: &'static str, value: &Value) -> Result<AttributeValue, AttributeError> {
    let wrong = || malformed(id, format!("{value} is not a valid blend_to_gray argument"));
    let fields = value.as_object().ok_or_else(wrong)?;
    let field = |name: &str| {
        let raw = fields.get(name).and_then(Value::as_f64).ok_or_else(wrong)? as f32;
        unit(id, name, raw)
    };
    Ok(AttributeValue::BlendToGray { brightness: field("brightness")?, factor: field("factor")? })
}

fn unit(id: &'static str, name: &str, value: f32) -> Result<f32, AttributeError> {
    if !(0.0..=1.0).contains(&value) {
        return Err(malformed(id, format!("`{name}` {value} is not in range [0; 1]")));
    }
    Ok(value)
}

fn parse_typed(
    id: &'static str,
    ty: AttributeType,
    value: &Value,
) -> Result<AttributeValue, AttributeError> {
    let wrong = || malformed(id, format!("{value} is not a valid {ty:?} value"));
    Ok(match ty {
        AttributeType::Boolean => AttributeValue::Bool(value.as_bool().ok_or_else(wrong)?),
        AttributeType::Float | AttributeType::AngleDegrees => {
            AttributeValue::Float(value.as_f64().ok_or_else(wrong)? as f32)
        }
        AttributeType::Integer => {
            AttributeValue::Integer(value.as_i64().ok_or_else(wrong)?.try_into().map_err(|_| wrong())?)
        }
        AttributeType::RgbColor => AttributeValue::Color(parse_color(value, 6).ok_or_else(wrong)?),
        AttributeType::ArgbColor => AttributeValue::Color(parse_color(value, 8).ok_or_else(wrong)?),
        AttributeType::AmbientParticles => {
            AttributeValue::List(value.as_array().ok_or_else(wrong)?.clone())
        }
        AttributeType::MobSpawnSettings => AttributeValue::MobSpawns(Box::new(
            MobSpawnSettings::deserialize(value).map_err(|e| malformed(id, e.to_string()))?,
        )),
        _ => AttributeValue::Opaque(value.clone()),
    })
}

/// `ExtraCodecs.STRING_RGB_COLOR` / `STRING_ARGB_COLOR`: a `#`-prefixed hex
/// string of `digits` length, a packed integer, or the float vector form —
/// three components for rgb, four for argb with the alpha last.
fn parse_color(value: &Value, digits: usize) -> Option<u32> {
    match value {
        Value::String(s) => {
            let hex = s.strip_prefix('#')?;
            if hex.len() != digits {
                return None;
            }
            let raw = u32::from_str_radix(hex, 16).ok()?;
            Some(if digits == 6 { raw | 0xFF00_0000 } else { raw })
        }
        Value::Number(n) => n.as_i64().map(|v| v as u32),
        Value::Array(components) => {
            let expected = if digits == 6 { 3 } else { 4 };
            if components.len() != expected {
                return None;
            }
            let channel = |i: usize| Some(as_8bit_channel(components[i].as_f64()? as f32));
            let alpha = if expected == 4 { channel(3)? } else { 255 };
            Some(alpha << 24 | channel(0)? << 16 | channel(1)? << 8 | channel(2)?)
        }
        _ => None,
    }
}

/// `ARGB.as8BitChannel`: floor, then the same truncation `ARGB.color` applies.
fn as_8bit_channel(value: f32) -> u32 {
    (value * 255.0).floor() as i32 as u32 & 0xFF
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
    use AttributeValue::*;

    let color = |packed: i32| Color(packed as u32);
    let bed_rule = |can_set_spawn, destroy_on_leave| {
        let mut fields = json!({
            "can_sleep": "when_dark",
            "can_set_spawn": can_set_spawn,
            "error_message": {"translate": "block.minecraft.bed.no_sleep"},
        });
        if destroy_on_leave {
            fields["destroy_on_leave"] = json!(true);
        }
        Opaque(fields)
    };

    vec![
        row("minecraft:visual/sky_color", T::RgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/fog_color", T::RgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/water_fog_color", T::RgbColor, color(-16448205), R::Any, SYNC | INTERP),
        row("minecraft:visual/sky_light_color", T::RgbColor, color(-1), R::Any, SYNC | INTERP),
        row("minecraft:visual/ambient_light_color", T::RgbColor, color(-16777216), R::Any, SYNC | INTERP),
        row("minecraft:visual/block_light_tint", T::RgbColor, color(-10100), R::Any, SYNC | INTERP),
        row("minecraft:visual/night_vision_color", T::RgbColor, color(-6710887), R::Any, SYNC | INTERP),
        row("minecraft:visual/cloud_color", T::ArgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/sunrise_sunset_color", T::ArgbColor, color(0), R::Any, SYNC | INTERP),
        row("minecraft:visual/cloud_height", T::Float, Float(192.33), R::Any, SYNC | INTERP),
        row("minecraft:visual/fog_start_distance", T::Float, Float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/fog_end_distance", T::Float, Float(1024.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/sky_fog_end_distance", T::Float, Float(512.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/cloud_fog_end_distance", T::Float, Float(2048.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/water_fog_start_distance", T::Float, Float(-8.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/water_fog_end_distance", T::Float, Float(96.0), R::NON_NEGATIVE, SYNC | INTERP),
        row("minecraft:visual/sky_light_factor", T::Float, Float(1.0), R::UNIT, SYNC | INTERP),
        row("minecraft:visual/star_brightness", T::Float, Float(0.0), R::UNIT, SYNC | INTERP),
        row("minecraft:visual/sun_angle", T::AngleDegrees, Float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/moon_angle", T::AngleDegrees, Float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/star_angle", T::AngleDegrees, Float(0.0), R::Any, SYNC | INTERP),
        row("minecraft:visual/moon_phase", T::MoonPhase, Opaque(json!("full_moon")), R::Any, SYNC),
        row("minecraft:visual/ambient_particles", T::AmbientParticles, List(Vec::new()), R::Any, SYNC),
        row("minecraft:visual/default_dripstone_particle", T::Particle, Opaque(json!({"type": "minecraft:dripping_dripstone_water"})), R::Any, SYNC),

        row("minecraft:audio/background_music", T::BackgroundMusic, Opaque(json!({})), R::Any, SYNC),
        row("minecraft:audio/ambient_sounds", T::AmbientSounds, Opaque(json!({})), R::Any, SYNC),
        row("minecraft:audio/music_volume", T::Float, Float(1.0), R::UNIT, SYNC),
        row("minecraft:audio/firefly_bush_sounds", T::Boolean, Bool(false), R::Any, SYNC),

        row("minecraft:gameplay/sky_light_level", T::Float, Float(15.0), R::Bounded { min: 0.0, max: 15.0 }, SYNC | NOT_POSITIONAL),
        row("minecraft:gameplay/fast_lava", T::Boolean, Bool(false), R::Any, SYNC | NOT_POSITIONAL),
        row("minecraft:gameplay/water_evaporates", T::Boolean, Bool(false), R::Any, SYNC),
        row("minecraft:gameplay/piglins_zombify", T::Boolean, Bool(true), R::Any, SYNC),
        row("minecraft:gameplay/creaking_active", T::Boolean, Bool(false), R::Any, SYNC),
        row("minecraft:gameplay/natural_mob_spawns", T::MobSpawnSettings, MobSpawns(Box::default()), R::Any, 0),
        row("minecraft:gameplay/can_start_raid", T::Boolean, Bool(true), R::Any, 0),
        row("minecraft:gameplay/can_pillager_patrol_spawn", T::Boolean, Bool(true), R::Any, 0),
        row("minecraft:gameplay/bees_stay_in_hive", T::Boolean, Bool(false), R::Any, 0),
        row("minecraft:gameplay/monsters_burn", T::Boolean, Bool(false), R::Any, 0),
        row("minecraft:gameplay/snow_golem_melts", T::Boolean, Bool(false), R::Any, 0),
        row("minecraft:gameplay/increased_fire_burnout", T::Boolean, Bool(false), R::Any, 0),
        row("minecraft:gameplay/nether_portal_spawns_piglin", T::Boolean, Bool(false), R::Any, 0),
        row("minecraft:gameplay/respawn_anchor_works", T::Boolean, Bool(false), R::Any, 0),
        row("minecraft:gameplay/eyeblossom_open", T::TriState, Opaque(json!("default")), R::Any, 0),
        row("minecraft:gameplay/bed_rule", T::BedRule, bed_rule("always", false), R::Any, 0),
        row("minecraft:gameplay/straw_bed_rule", T::BedRule, bed_rule("never", true), R::Any, 0),
        row("minecraft:gameplay/villager_activity", T::Activity, Opaque(json!("minecraft:idle")), R::Any, 0),
        row("minecraft:gameplay/baby_villager_activity", T::Activity, Opaque(json!("minecraft:idle")), R::Any, 0),
        row("minecraft:gameplay/cat_waking_up_gift_chance", T::Float, Float(0.0), R::UNIT, 0),
        row("minecraft:gameplay/surface_slime_spawn_chance", T::Float, Float(0.0), R::UNIT, 0),
        row("minecraft:gameplay/turtle_egg_hatch_chance", T::Float, Float(0.002), R::UNIT, 0),
        row("minecraft:gameplay/creature_world_gen_spawn_probability", T::Float, Float(0.1), R::UNIT_EPSILON, 0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_holds_every_attribute() {
        assert_eq!(ENVIRONMENT_ATTRIBUTES.len(), 51);
        assert_eq!(table().len(), 51, "ids must be unique");

        let syncable: Vec<_> =
            ENVIRONMENT_ATTRIBUTES.values().filter(|spec| spec.syncable).collect();
        assert_eq!(syncable.len(), 33);
        assert!(
            syncable
                .iter()
                .filter(|spec| spec.id.starts_with("minecraft:gameplay/"))
                .count()
                == 5
        );

        for spec in ENVIRONMENT_ATTRIBUTES.values() {
            assert!(spec.id.starts_with("minecraft:"), "{} is not namespaced", spec.id);
        }
    }

    #[test]
    fn flags_match_the_reference() {
        let sky_color = attribute("minecraft:visual/sky_color").unwrap();
        assert!(sky_color.syncable && sky_color.positional && sky_color.spatially_interpolated);

        let sky_light_level = attribute("minecraft:gameplay/sky_light_level").unwrap();
        assert!(sky_light_level.syncable);
        assert!(!sky_light_level.positional);

        assert!(!is_syncable("minecraft:gameplay/natural_mob_spawns"));
        assert!(is_syncable("minecraft:audio/music_volume"));
        assert!(!is_syncable("minecraft:visual/not_an_attribute"));
    }

    #[test]
    fn parses_colors_and_ranges() {
        let sky_color = attribute("minecraft:visual/sky_color").unwrap();
        assert_eq!(
            sky_color.parse_value(&json!("#78a7ff")).unwrap(),
            AttributeValue::Color(0xFF78_A7FF)
        );
        assert!(sky_color.parse_value(&json!("#ccffffff")).is_err(), "rgb takes 6 digits");

        let cloud_color = attribute("minecraft:visual/cloud_color").unwrap();
        assert_eq!(
            cloud_color.parse_value(&json!("#ccffffff")).unwrap(),
            AttributeValue::Color(0xCCFF_FFFF)
        );

        let volume = attribute("minecraft:audio/music_volume").unwrap();
        assert_eq!(volume.parse_value(&json!(0.5)).unwrap(), AttributeValue::Float(0.5));
        assert!(volume.parse_value(&json!(1.5)).is_err(), "music_volume is UNIT");
    }

    #[test]
    fn colors_accept_the_float_vector_form() {
        let sky_color = attribute("minecraft:visual/sky_color").unwrap();
        let cloud_color = attribute("minecraft:visual/cloud_color").unwrap();

        assert_eq!(
            sky_color.parse_value(&json!([1.0, 0.5, 0.0])).unwrap(),
            AttributeValue::Color(0xFFFF_7F00)
        );
        // the fourth component is the alpha
        assert_eq!(
            cloud_color.parse_value(&json!([1.0, 0.5, 0.0, 0.5])).unwrap(),
            AttributeValue::Color(0x7FFF_7F00)
        );
        assert!(sky_color.parse_value(&json!([1.0, 0.5, 0.0, 0.5])).is_err(), "rgb takes 3");
        assert!(cloud_color.parse_value(&json!([1.0, 0.5, 0.0])).is_err(), "argb takes 4");
    }

    #[test]
    fn multiply_argument_escapes_the_attribute_range() {
        // FloatModifier.Simple validates the argument as a plain float, so 0.85
        // is legal here even though the attribute itself is NON_NEGATIVE.
        let end = attribute("minecraft:visual/water_fog_end_distance").unwrap();
        assert_eq!(
            end.parse_argument(Operation::Multiply, &json!(0.85)).unwrap(),
            AttributeValue::Float(0.85)
        );
        assert!(end.parse_argument(Operation::Or, &json!(true)).is_err());
    }

    #[test]
    fn color_add_takes_the_six_digit_form_on_both_colour_types() {
        let sky_color = attribute("minecraft:visual/sky_color").unwrap();
        let cloud_color = attribute("minecraft:visual/cloud_color").unwrap();
        for spec in [sky_color, cloud_color] {
            assert_eq!(
                spec.parse_argument(Operation::Add, &json!("#102030")).unwrap(),
                AttributeValue::Color(0xFF10_2030),
                "{} takes the six-digit form for add",
                spec.id
            );
            assert!(
                spec.parse_argument(Operation::Subtract, &json!("#80102030")).is_err(),
                "{} must reject an eight-digit subtract argument",
                spec.id
            );
        }
        assert_eq!(
            cloud_color.parse_argument(Operation::Multiply, &json!("#80102030")).unwrap(),
            AttributeValue::Color(0x8010_2030)
        );
        assert!(sky_color.parse_argument(Operation::Multiply, &json!("#80102030")).is_err());
    }
}
