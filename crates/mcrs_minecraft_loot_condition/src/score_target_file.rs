use serde::Deserialize;

use crate::entity_target::EntityTarget;

/// The `target` of `score` (REG-FLD-738): a bare entity target, or an object of the type
/// `fixed` or `context`.
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
