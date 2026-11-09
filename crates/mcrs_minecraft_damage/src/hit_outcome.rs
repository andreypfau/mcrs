/// What a hit did (DMG-HIT-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HitOutcome {
    Rejected,
    Partial,
    Full,
}
