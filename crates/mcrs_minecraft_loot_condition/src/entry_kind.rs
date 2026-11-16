use crate::member_kind::MemberKind;

/// The registry of a named entry: `predicate`, `context_int_provider` or
/// `context_float_provider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Predicate,
    Int,
    Float,
}

impl EntryKind {
    pub(crate) const fn member(self) -> MemberKind {
        match self {
            EntryKind::Predicate => MemberKind::Predicate,
            EntryKind::Int => MemberKind::IntProvider,
            EntryKind::Float => MemberKind::FloatProvider,
        }
    }
}
