use mcrs_minecraft_core::codec::NonNegativeInt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weighted<T> {
    pub data: T,
    pub weight: NonNegativeInt,
}
