use crate::precise_sync::PreciseSync;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClientMotion {
    Replayed,
    Simulated(PreciseSync),
}
