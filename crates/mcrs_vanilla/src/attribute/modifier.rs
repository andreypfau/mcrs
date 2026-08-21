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
    #[error("{op:?} on {ty:?} is not implemented")]
    Unimplemented { ty: AttributeType, op: Operation },
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
        (Operation::Multiply, V::Float(a), V::Float(b)) => Ok(V::Float(a * b)),
        (Operation::Multiply, V::Integer(a), V::Integer(b)) => Ok(V::Integer(a * b)),
        (Operation::Multiply, V::Color(a), V::Color(b)) => Ok(V::Color(multiply_color(*a, *b))),
        (Operation::Maximum, V::Float(a), V::Float(b)) => Ok(V::Float(a.max(*b))),
        (Operation::Maximum, V::Integer(a), V::Integer(b)) => Ok(V::Integer(*a.max(b))),
        (Operation::Or, V::Bool(a), V::Bool(b)) => Ok(V::Bool(*a || *b)),
        (Operation::And, V::Bool(a), V::Bool(b)) => Ok(V::Bool(*a && *b)),
        (Operation::Append, V::List(a), V::List(b)) => Ok(V::List([a.clone(), b.clone()].concat())),
        (Operation::Overlay, V::MobSpawns(a), V::MobSpawns(b)) => {
            Ok(V::MobSpawns(Box::new(overlay_spawns(a, b))))
        }
        (
            Operation::Multiply
            | Operation::Maximum
            | Operation::Or
            | Operation::And
            | Operation::Append
            | Operation::Overlay,
            _,
            _,
        ) => Err(mismatch()),
        _ => Err(ModifierError::Unimplemented { ty, op }),
    }
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
    use crate::attribute::registry::{attribute, parse_argument, parse_value};
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
        let base = parse_value(spec, &base).unwrap();
        let argument = parse_argument(spec, op, &argument).unwrap();
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

        let err = apply(
            AttributeType::RgbColor,
            Operation::BlendToGray,
            &AttributeValue::Color(0),
            &AttributeValue::Opaque(json!({"brightness": 0.5, "factor": 0.5})),
        );
        assert!(matches!(err, Err(ModifierError::Unimplemented { .. })));
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
