use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemberKind {
    Predicate,
    IntProvider,
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
