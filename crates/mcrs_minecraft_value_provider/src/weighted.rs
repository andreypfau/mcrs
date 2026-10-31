use mcrs_minecraft_core::codec::NonNegativeInt;
use serde::{Deserialize, Serialize};

/// One entry of a `WeightedList`: the value under `data`, its share under `weight`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weighted<T> {
    pub data: T,
    pub weight: NonNegativeInt,
}
