use crate::IntProvider;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// A provider whose whole range must sit inside `[MIN, MAX]`, refused at load
/// otherwise.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BoundedIntProvider<const MIN: i32, const MAX: i32>(pub IntProvider);

impl<const MIN: i32, const MAX: i32> std::ops::Deref for BoundedIntProvider<MIN, MAX> {
    type Target = IntProvider;

    fn deref(&self) -> &IntProvider {
        &self.0
    }
}

impl<'de, const MIN: i32, const MAX: i32> Deserialize<'de> for BoundedIntProvider<MIN, MAX> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let provider = IntProvider::deserialize(deserializer)?;
        let (low, high) = provider.bounds();
        if low < MIN {
            return Err(D::Error::custom(format!(
                "Value provider too low: {MIN} [{low}-{high}]"
            )));
        }
        if high > MAX {
            return Err(D::Error::custom(format!(
                "Value provider too high: {MAX} [{low}-{high}]"
            )));
        }
        Ok(BoundedIntProvider(provider))
    }
}
