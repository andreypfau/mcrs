/// When an update of a simulated entity sends the absolute position although a relative move
/// fits (`entity-tracking.md`, TRK-UPD-012 and TRK-UPD-013).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreciseSync {
    Never,
    /// After each tick whose move had a horizontal or a vertical collision.
    AfterCollision,
    /// When the move along the face of a collision loses precision on the wire.
    LostPrecision,
}
