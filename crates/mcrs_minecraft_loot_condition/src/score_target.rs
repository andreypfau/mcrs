use crate::context_param::ContextParam;

/// The score holder of `score` (LOOT-NUM-019).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScoreTarget {
    Entity(ContextParam),
    Name(Box<str>),
}
