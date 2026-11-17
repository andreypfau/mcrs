use crate::context_param::ContextParam;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScoreTarget {
    Entity(ContextParam),
    Name(Box<str>),
}
