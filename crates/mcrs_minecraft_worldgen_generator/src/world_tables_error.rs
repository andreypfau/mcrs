#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WorldTablesError {
    #[error("the loaded registries cannot list their spawn conditions: {0}")]
    SpawnConditions(String),
    #[error("the structure registries do not resolve: {0}")]
    StructureFreeze(String),
}
