use mcrs_minecraft_enchantment::level_based_value::{DispatchedLevelBasedValue, LevelBasedValue};
use serde::de::Error;
use serde::{Deserialize, Deserializer};

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
