//! `AttributeModifier`: the operations that compose one attribute layer onto
//! the layer beneath it.
//!
//! An operation means different things for different attribute types — RGB
//! `multiply` is a per-channel colour multiply, float `multiply` is plain
//! multiplication — so which operations exist at all is decided by
//! [`AttributeType::allows`] and the arithmetic by the value the type parses to.

use serde::{Deserialize, Serialize};

use super::registry::{AttributeType, AttributeValue};
use crate::biome::{MobSpawnSettings, SpawnCost};
use crate::ResourceLocation;
use std::collections::BTreeMap;
use std::sync::Arc;

/// `AttributeModifier.OperationId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Override,
    AlphaBlend,
    Add,
    Subtract,
    Multiply,
    BlendToGray,
    Minimum,
    Maximum,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Append,
    Overlay,
}

#[derive(Debug, thiserror::Error)]
pub enum ModifierError {
    #[error("{op:?} is not a valid modifier for {ty:?}")]
    NotAllowed { ty: AttributeType, op: Operation },
    #[error("{op:?} on {ty:?} was given values it cannot combine")]
    Mismatch { ty: AttributeType, op: Operation },
}

/// Compose `argument` onto `subject` and return the result.
///
/// Pure: neither input is touched and nothing is cached. The composed value
/// belongs to whoever asked for it.
pub fn apply(
    ty: AttributeType,
    op: Operation,
    subject: &AttributeValue,
    argument: &AttributeValue,
) -> Result<AttributeValue, ModifierError> {
    use AttributeValue as V;

    if !ty.allows(op) {
        return Err(ModifierError::NotAllowed { ty, op });
    }
    let mismatch = || ModifierError::Mismatch { ty, op };
    match (op, subject, argument) {
        (Operation::Override, _, argument) => Ok(argument.clone()),

        (Operation::Add, V::Float(a), V::Float(b)) => Ok(V::Float(a + b)),
        (Operation::Subtract, V::Float(a), V::Float(b)) => Ok(V::Float(a - b)),
        (Operation::Multiply, V::Float(a), V::Float(b)) => Ok(V::Float(a * b)),
        (Operation::Minimum, V::Float(a), V::Float(b)) => Ok(V::Float(a.min(*b))),
        (Operation::Maximum, V::Float(a), V::Float(b)) => Ok(V::Float(a.max(*b))),
        (Operation::AlphaBlend, V::Float(a), V::FloatWithAlpha { value, alpha }) => {
            Ok(V::Float(a + alpha * (value - a)))
        }

        // Java int arithmetic wraps.
        (Operation::Add, V::Integer(a), V::Integer(b)) => Ok(V::Integer(a.wrapping_add(*b))),
        (Operation::Subtract, V::Integer(a), V::Integer(b)) => Ok(V::Integer(a.wrapping_sub(*b))),
        (Operation::Multiply, V::Integer(a), V::Integer(b)) => Ok(V::Integer(a.wrapping_mul(*b))),
        (Operation::Minimum, V::Integer(a), V::Integer(b)) => Ok(V::Integer(*a.min(b))),
        (Operation::Maximum, V::Integer(a), V::Integer(b)) => Ok(V::Integer(*a.max(b))),

        (Operation::Add, V::Color(a), V::Color(b)) => Ok(V::Color(add_rgb(*a, *b))),
        (Operation::Subtract, V::Color(a), V::Color(b)) => Ok(V::Color(subtract_rgb(*a, *b))),
        (Operation::Multiply, V::Color(a), V::Color(b)) => Ok(V::Color(multiply_color(*a, *b))),
        (Operation::AlphaBlend, V::Color(a), V::Color(b)) => Ok(V::Color(alpha_blend(*a, *b))),
        (Operation::BlendToGray, V::Color(a), V::BlendToGray { brightness, factor }) => {
            Ok(V::Color(blend_to_gray(*a, *brightness, *factor)))
        }

        (Operation::And, V::Bool(a), V::Bool(b)) => Ok(V::Bool(*a && *b)),
        (Operation::Nand, V::Bool(a), V::Bool(b)) => Ok(V::Bool(!(*a && *b))),
        (Operation::Or, V::Bool(a), V::Bool(b)) => Ok(V::Bool(*a || *b)),
        (Operation::Nor, V::Bool(a), V::Bool(b)) => Ok(V::Bool(!(*a || *b))),
        (Operation::Xor, V::Bool(a), V::Bool(b)) => Ok(V::Bool(a != b)),
        (Operation::Xnor, V::Bool(a), V::Bool(b)) => Ok(V::Bool(a == b)),

        (Operation::Append, V::List(a), V::List(b)) => Ok(V::List([a.clone(), b.clone()].concat())),
        (Operation::Overlay, V::MobSpawns(a), V::MobSpawns(b)) => {
            Ok(V::MobSpawns(Box::new(overlay_spawns(a, b))))
        }

        _ => Err(mismatch()),
    }
}

/// `ARGB.addRgb` / `subtractRgb`: per channel and clamped, keeping the
/// subject's alpha — the argument's is not read.
fn add_rgb(lhs: u32, rhs: u32) -> u32 {
    zip_rgb(lhs, rhs, |a, b| (a + b).min(255))
}

fn subtract_rgb(lhs: u32, rhs: u32) -> u32 {
    zip_rgb(lhs, rhs, |a, b| a.saturating_sub(b))
}

fn zip_rgb(lhs: u32, rhs: u32, channel: impl Fn(u32, u32) -> u32) -> u32 {
    let at = |color: u32, shift: u32| color >> shift & 0xFF;
    lhs & 0xFF00_0000
        | channel(at(lhs, 16), at(rhs, 16)) << 16
        | channel(at(lhs, 8), at(rhs, 8)) << 8
        | channel(at(lhs, 0), at(rhs, 0))
}

/// `ARGB.alphaBlend`: `source` over `destination`, premultiplied by the
/// source's alpha.
fn alpha_blend(destination: u32, source: u32) -> u32 {
    let at = |color: u32, shift: u32| color >> shift & 0xFF;
    let (destination_alpha, source_alpha) = (at(destination, 24), at(source, 24));
    if source_alpha == 255 {
        return source;
    }
    if source_alpha == 0 {
        return destination;
    }
    let alpha = source_alpha + destination_alpha * (255 - source_alpha) / 255;
    let channel = |shift: u32| {
        (at(source, shift) * source_alpha + at(destination, shift) * (alpha - source_alpha)) / alpha
    };
    alpha << 24 | channel(16) << 16 | channel(8) << 8 | channel(0)
}

/// `ColorModifier.BLEND_TO_GRAY`: scale the luminance of `subject` by
/// `brightness`, then lerp towards it by `factor`.
fn blend_to_gray(subject: u32, brightness: f32, factor: f32) -> u32 {
    let at = |color: u32, shift: u32| color >> shift & 0xFF;
    let luminance = (at(subject, 16) as f32 * 0.3
        + at(subject, 8) as f32 * 0.59
        + at(subject, 0) as f32 * 0.11) as u32;
    let scaled = ((luminance as f32 * brightness) as u32).min(255);
    let lerp = |from: u32, to: u32| {
        (from as i32 + (factor * (to as i32 - from as i32) as f32).floor() as i32) as u32
    };
    at(subject, 24) << 24
        | lerp(at(subject, 16), scaled) << 16
        | lerp(at(subject, 8), scaled) << 8
        | lerp(at(subject, 0), scaled)
}

/// `ARGB.multiply`: per-channel, alpha included. `-1` (opaque white) is the
/// identity on both sides.
fn multiply_color(lhs: u32, rhs: u32) -> u32 {
    if lhs == u32::MAX {
        return rhs;
    }
    if rhs == u32::MAX {
        return lhs;
    }
    let channel = |shift: u32| ((lhs >> shift & 0xFF) * (rhs >> shift & 0xFF) / 255) << shift;
    channel(24) | channel(16) | channel(8) | channel(0)
}

/// `MobSpawnSettingsModifier.Overlay`: a category defined by `second` replaces
/// the one in `first` wholesale; spawn costs merge with `second` winning.
fn overlay_spawns(first: &MobSpawnSettings, second: &MobSpawnSettings) -> MobSpawnSettings {
    if first.is_empty() {
        return second.clone();
    }
    if second.is_empty() {
        return first.clone();
    }
    let mut spawns_by_category = first.spawns_by_category.clone();
    spawns_by_category.extend(
        second.spawns_by_category.iter().map(|(category, data)| (*category, data.clone())),
    );

    let mut spawn_costs: BTreeMap<ResourceLocation<Arc<str>>, SpawnCost> =
        first.spawn_costs.clone();
    spawn_costs.extend(second.spawn_costs.iter().map(|(id, cost)| (id.clone(), cost.clone())));

    MobSpawnSettings { spawn_costs, spawns_by_category }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::registry::attribute;
    use serde_json::json;

    fn spec(id: &str) -> &'static super::super::registry::AttributeSpec {
        attribute(id).unwrap()
    }

    /// Apply an entry the way a layer will: parse the base value and the
    /// argument through the registry, then compose them.
    fn compose(
        id: &str,
        base: serde_json::Value,
        op: Operation,
        argument: serde_json::Value,
    ) -> AttributeValue {
        let spec = spec(id);
        let base = spec.parse_value(&base).unwrap();
        let argument = spec.parse_argument(op, &argument).unwrap();
        apply(spec.ty, op, &base, &argument).unwrap()
    }

    #[test]
    fn override_replaces() {
        assert_eq!(
            compose("minecraft:visual/sky_color", json!("#000000"), Operation::Override, json!("#78a7ff")),
            AttributeValue::Color(0xFF78_A7FF)
        );
    }

    #[test]
    fn multiply_is_per_type() {
        // FLOAT: plain multiplication — the shipped swamp/mangrove case.
        assert_eq!(
            compose(
                "minecraft:visual/water_fog_end_distance",
                json!(96.0),
                Operation::Multiply,
                json!(0.85)
            ),
            AttributeValue::Float(96.0f32 * 0.85f32)
        );
        // RGB_COLOR: per channel, and opaque white is the identity.
        assert_eq!(
            compose(
                "minecraft:visual/sky_color",
                json!("#80ff40"),
                Operation::Multiply,
                json!("#ffffff")
            ),
            AttributeValue::Color(0xFF80_FF40)
        );
        assert_eq!(multiply_color(0xFF80_8080, 0xFF80_8080), 0xFF40_4040);
        // ARGB_COLOR carries the alpha channel through the same multiply.
        assert_eq!(multiply_color(0x8080_8080, 0xFFFF_FFFF), 0x8080_8080);
    }

    #[test]
    fn maximum_takes_the_larger() {
        assert_eq!(
            compose(
                "minecraft:gameplay/cat_waking_up_gift_chance",
                json!(0.3),
                Operation::Maximum,
                json!(0.7)
            ),
            AttributeValue::Float(0.7)
        );
    }

    #[test]
    fn boolean_or_and_and() {
        assert_eq!(
            compose("minecraft:gameplay/creaking_active", json!(false), Operation::Or, json!(true)),
            AttributeValue::Bool(true)
        );
        assert_eq!(
            compose(
                "minecraft:gameplay/can_pillager_patrol_spawn",
                json!(true),
                Operation::And,
                json!(false)
            ),
            AttributeValue::Bool(false)
        );
    }

    #[test]
    fn append_concatenates_particles() {
        let ash = json!([{"particle": {"type": "minecraft:ash"}, "probability": 0.00625}]);
        let white = json!([{"particle": {"type": "minecraft:white_ash"}, "probability": 0.118}]);
        let AttributeValue::List(joined) =
            compose("minecraft:visual/ambient_particles", ash, Operation::Append, white)
        else {
            panic!("append yields a list");
        };
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[1]["particle"]["type"], json!("minecraft:white_ash"));
    }

    #[test]
    fn overlay_replaces_defined_categories() {
        let base = json!({
            "spawns_by_category": {
                "monster": [{"type": "minecraft:zombie", "count": 4, "weight": 95}],
                "creature": [{"type": "minecraft:cow", "count": 4, "weight": 8}]
            },
            "spawn_costs": {"minecraft:zombie": {"charge": 1.0, "energy_budget": 0.5}}
        });
        let overlay = json!({
            "spawns_by_category": {"monster": []},
            "spawn_costs": {"minecraft:warden": {"charge": 0.7, "energy_budget": 0.15}}
        });
        let AttributeValue::MobSpawns(result) = compose(
            "minecraft:gameplay/natural_mob_spawns",
            base,
            Operation::Overlay,
            overlay,
        ) else {
            panic!("overlay yields spawn settings");
        };

        // deep_dark defines `monster: []` precisely to suppress the layer below.
        assert_eq!(result.spawns_by_category[&crate::biome::MobCategory::Monster].len(), 0);
        assert_eq!(result.spawns_by_category[&crate::biome::MobCategory::Creature].len(), 1);
        assert_eq!(result.spawn_costs.len(), 2);
    }

    #[test]
    fn rejects_operations_the_type_does_not_have() {
        let err = apply(
            AttributeType::Boolean,
            Operation::Multiply,
            &AttributeValue::Bool(true),
            &AttributeValue::Bool(true),
        );
        assert!(matches!(err, Err(ModifierError::NotAllowed { .. })));

        // blend_to_gray is allowed on a colour, but not with a raw payload
        let err = apply(
            AttributeType::RgbColor,
            Operation::BlendToGray,
            &AttributeValue::Color(0),
            &AttributeValue::Opaque(json!({"brightness": 0.5, "factor": 0.5})),
        );
        assert!(matches!(err, Err(ModifierError::Mismatch { .. })));
    }

    #[test]
    fn float_arithmetic_matches_the_reference() {
        let distance = "minecraft:visual/fog_end_distance";
        assert_eq!(
            compose(distance, json!(100.0), Operation::Add, json!(25.0)),
            AttributeValue::Float(125.0)
        );
        assert_eq!(
            compose(distance, json!(100.0), Operation::Subtract, json!(25.0)),
            AttributeValue::Float(75.0)
        );
        assert_eq!(
            compose(distance, json!(100.0), Operation::Minimum, json!(25.0)),
            AttributeValue::Float(25.0)
        );
        // FloatWithAlpha lerps from the subject towards the argument
        assert_eq!(
            compose(
                distance,
                json!(100.0),
                Operation::AlphaBlend,
                json!({"value": 200.0, "alpha": 0.25})
            ),
            AttributeValue::Float(125.0)
        );
        // a bare float means alpha 1, so the argument wins outright
        assert_eq!(
            compose(distance, json!(100.0), Operation::AlphaBlend, json!(200.0)),
            AttributeValue::Float(200.0)
        );
    }

    #[test]
    fn colour_arithmetic_matches_the_reference() {
        let sky = "minecraft:visual/sky_color";
        // add and subtract clamp per channel and keep the subject's alpha
        assert_eq!(
            compose(sky, json!("#80ff40"), Operation::Add, json!("#4010c0")),
            AttributeValue::Color(0xFFC0_FFFF)
        );
        assert_eq!(
            compose(sky, json!("#80ff40"), Operation::Subtract, json!("#40ff80")),
            AttributeValue::Color(0xFF40_0000)
        );
        // an opaque source replaces the destination outright
        assert_eq!(
            compose(sky, json!("#102030"), Operation::AlphaBlend, json!("#ff405060")),
            AttributeValue::Color(0xFF40_5060)
        );
        // a fully transparent source leaves it alone
        assert_eq!(
            compose(sky, json!("#102030"), Operation::AlphaBlend, json!("#00405060")),
            AttributeValue::Color(0xFF10_2030)
        );
        // factor 1 lands on the scaled greyscale, factor 0 leaves the subject
        assert_eq!(
            compose(
                sky,
                json!("#646464"),
                Operation::BlendToGray,
                json!({"brightness": 0.5, "factor": 1.0})
            ),
            AttributeValue::Color(0xFF32_3232)
        );
        assert_eq!(
            compose(
                sky,
                json!("#646464"),
                Operation::BlendToGray,
                json!({"brightness": 0.5, "factor": 0.0})
            ),
            AttributeValue::Color(0xFF64_6464)
        );
    }

    #[test]
    fn every_boolean_operation_is_implemented() {
        let creaking = "minecraft:gameplay/creaking_active";
        let cases = [
            (Operation::And, false, true, false),
            (Operation::Nand, false, true, true),
            (Operation::Or, false, true, true),
            (Operation::Nor, false, true, false),
            (Operation::Xor, true, true, false),
            (Operation::Xnor, true, true, true),
        ];
        for (op, subject, argument, expected) in cases {
            assert_eq!(
                compose(creaking, json!(subject), op, json!(argument)),
                AttributeValue::Bool(expected),
                "{op:?} {subject} {argument}"
            );
        }
    }

    #[test]
    fn every_operation_has_an_implementation() {
        // No operation may reach `apply` and find nothing to do: the arguments
        // below are the shape each one's codec produces.
        use AttributeValue as V;
        let cases: [(AttributeType, Operation, V, V); 16] = [
            (AttributeType::Boolean, Operation::Override, V::Bool(false), V::Bool(true)),
            (AttributeType::Boolean, Operation::And, V::Bool(true), V::Bool(true)),
            (AttributeType::Boolean, Operation::Nand, V::Bool(true), V::Bool(true)),
            (AttributeType::Boolean, Operation::Or, V::Bool(true), V::Bool(true)),
            (AttributeType::Boolean, Operation::Nor, V::Bool(true), V::Bool(true)),
            (AttributeType::Boolean, Operation::Xor, V::Bool(true), V::Bool(true)),
            (AttributeType::Boolean, Operation::Xnor, V::Bool(true), V::Bool(true)),
            (AttributeType::Float, Operation::Add, V::Float(1.0), V::Float(2.0)),
            (AttributeType::Float, Operation::Subtract, V::Float(1.0), V::Float(2.0)),
            (AttributeType::Float, Operation::Multiply, V::Float(1.0), V::Float(2.0)),
            (AttributeType::Float, Operation::Minimum, V::Float(1.0), V::Float(2.0)),
            (AttributeType::Float, Operation::Maximum, V::Float(1.0), V::Float(2.0)),
            (
                AttributeType::Float,
                Operation::AlphaBlend,
                V::Float(1.0),
                V::FloatWithAlpha { value: 2.0, alpha: 0.5 },
            ),
            (
                AttributeType::RgbColor,
                Operation::BlendToGray,
                V::Color(0xFF80_8080),
                V::BlendToGray { brightness: 0.5, factor: 0.5 },
            ),
            (
                AttributeType::AmbientParticles,
                Operation::Append,
                V::List(Vec::new()),
                V::List(Vec::new()),
            ),
            (
                AttributeType::MobSpawnSettings,
                Operation::Overlay,
                V::MobSpawns(Box::default()),
                V::MobSpawns(Box::default()),
            ),
        ];
        for (ty, op, subject, argument) in cases {
            assert!(apply(ty, op, &subject, &argument).is_ok(), "{ty:?} {op:?}");
        }
    }

    #[test]
    fn operation_ids_round_trip() {
        assert_eq!(serde_json::to_value(Operation::BlendToGray).unwrap(), json!("blend_to_gray"));
        assert_eq!(
            serde_json::from_value::<Operation>(json!("alpha_blend")).unwrap(),
            Operation::AlphaBlend
        );
    }
}
