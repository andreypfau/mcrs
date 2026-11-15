use mcrs_minecraft_enchantment::level_based_value::{DispatchedLevelBasedValue, LevelBasedValue};
use mcrs_minecraft_numeric::{clamp_f32, pow_f32};
use serde::de::Error;
use serde::{Deserialize, Deserializer};

/// A level-based value whose `clamped` parts have `max > min` (REG-FLD-387).
pub(crate) fn checked_level_based<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<LevelBasedValue, D::Error> {
    let value = LevelBasedValue::deserialize(deserializer)?;
    match well_formed(&value) {
        true => Ok(value),
        false => Err(D::Error::custom("a clamped value with max <= min")),
    }
}

fn well_formed(value: &LevelBasedValue) -> bool {
    use DispatchedLevelBasedValue as V;
    match value {
        LevelBasedValue::Constant(_) => true,
        LevelBasedValue::Dispatched(dispatched) => match dispatched {
            V::Clamped { value, min, max } => max > min && well_formed(value),
            V::Fraction {
                numerator: first,
                denominator: second,
            }
            | V::Exponent {
                base: first,
                power: second,
            } => well_formed(first) && well_formed(second),
            V::Lookup { fallback, .. } => well_formed(fallback),
            V::LevelsSquared { .. } | V::Linear { .. } => true,
        },
    }
}

/// The value of `value` at the level `level` (LOOT-NUM-021).
#[expect(
    clippy::cast_precision_loss,
    reason = "toF32 of an i32 is the nearest f32"
)]
pub(crate) fn level_based(value: &LevelBasedValue, level: i32) -> f32 {
    use DispatchedLevelBasedValue as V;
    let dispatched = match value {
        LevelBasedValue::Constant(value) => return *value,
        LevelBasedValue::Dispatched(dispatched) => dispatched,
    };
    match dispatched {
        V::Linear {
            base,
            per_level_above_first,
        } => base + per_level_above_first * level.wrapping_sub(1) as f32,
        V::LevelsSquared { added } => level.wrapping_mul(level) as f32 + added,
        V::Clamped { value, min, max } => clamp_f32(level_based(value, level), *min, *max),
        V::Fraction {
            numerator,
            denominator,
        } => {
            let denominator = level_based(denominator, level);
            match denominator == 0.0 {
                true => 0.0,
                false => level_based(numerator, level) / denominator,
            }
        }
        V::Exponent { base, power } => pow_f32(level_based(base, level), level_based(power, level)),
        V::Lookup { values, fallback } => usize::try_from(level)
            .ok()
            .and_then(|level| level.checked_sub(1))
            .and_then(|index| values.get(index).copied())
            .unwrap_or_else(|| level_based(fallback, level)),
    }
}
