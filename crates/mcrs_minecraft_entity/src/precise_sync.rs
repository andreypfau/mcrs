/// When an update of a simulated entity sends the absolute position although a relative move
/// fits (`entity-tracking.md`, TRK-UPD-012 and TRK-UPD-013).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreciseSync {
    /// Only for the reasons of every simulated entity: a resync or a move that does not fit.
    Never,
    /// After each tick whose move had a horizontal or a vertical collision.
    AfterCollision,
    /// When the move along the face of a collision loses precision on the wire.
    LostPrecision,
}
