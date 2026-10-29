use mcrs_minecraft_core::{ResourceLocation, rl};
use serde::{Deserialize, Serialize};

/// The default of a jigsaw's name and target: labels of attachment points, not registry entries.
pub const EMPTY_LABEL: ResourceLocation<&'static str> = rl!("minecraft:empty");

/// Whether a jigsaw may turn about its facing when it joins another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Joint {
    Rollable,
    Aligned,
}
