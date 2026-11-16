use core::fmt;

/// The registry of a named entry (LOOT-CTX-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemberKind {
    /// An entry of the registry `predicate`.
    Predicate,
    /// An entry of the registry `context_int_provider`.
    IntProvider,
    /// An entry of the registry `context_float_provider`.
    FloatProvider,
}

impl fmt::Display for MemberKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MemberKind::Predicate => "predicate",
            MemberKind::IntProvider => "int provider",
            MemberKind::FloatProvider => "float provider",
        })
    }
}
