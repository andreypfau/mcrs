#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreciseSync {
    Never,
    AfterCollision,
    LostPrecision,
}
