#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HitOutcome {
    Rejected,
    Partial,
    Full,
}
