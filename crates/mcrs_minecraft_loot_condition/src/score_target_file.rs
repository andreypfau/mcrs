use serde::Deserialize;

use crate::condition_baker::entity_target;
use crate::context_param::ContextParam;

/// The `target` of `score` (REG-FLD-738): a bare entity target, or an object of the type
/// `fixed` or `context`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum ScoreTargetFile {
    Bare(EntityTargetFile),
    Typed(TypedScoreTarget),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum TypedScoreTarget {
    #[serde(rename = "minecraft:fixed", alias = "fixed")]
    Fixed { name: String },
    #[serde(rename = "minecraft:context", alias = "context")]
    Context { target: EntityTargetFile },
}

/// An entity target (LOOT-CND-020), as the parameter that it names.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct EntityTargetFile(pub(crate) ContextParam);

impl TryFrom<String> for EntityTargetFile {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        entity_target(&text)
            .map(EntityTargetFile)
            .ok_or_else(|| format!("no entity target {text}"))
    }
}
