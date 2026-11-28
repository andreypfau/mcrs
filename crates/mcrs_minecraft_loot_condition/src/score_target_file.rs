use serde::Deserialize;

use crate::entity_target::EntityTarget;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum ScoreTargetFile {
    Bare(EntityTarget),
    Typed(TypedScoreTarget),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum TypedScoreTarget {
    #[serde(rename = "minecraft:fixed", alias = "fixed")]
    Fixed { name: String },
    #[serde(rename = "minecraft:context", alias = "context")]
    Context { target: EntityTarget },
}
