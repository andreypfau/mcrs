use mcrs_minecraft_resource::{ResourceLocation, rl};
use serde::{Deserialize, Serialize};

pub const EMPTY_LABEL: ResourceLocation<'static> = rl!("minecraft:empty");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Joint {
    Rollable,
    Aligned,
}
