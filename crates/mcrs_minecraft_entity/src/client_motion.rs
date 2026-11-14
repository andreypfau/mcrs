use crate::precise_sync::PreciseSync;

/// How the vanilla client moves an entity between two updates of the server
/// (`entity-tracking.md`, TRK-TYPE-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClientMotion {
    /// The client plays the path that each update sends.
    Replayed,
    /// The client simulates the entity from the last state that the server sent.
    Simulated(PreciseSync),
}
